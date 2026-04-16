/// Google status adapter for wtch.
///
/// Fetches a Google-style status dashboard that provides two endpoints:
/// - `{base_url}/products.json` — list of all services
/// - `{base_url}/incidents.json` — list of incidents (active if `end` is null)
///
/// Produces one component per product, marking those with active incidents.
use serde::Deserialize;

use crate::config::Watch;
use crate::state::WatchStatus;

use super::{AdapterError, AdapterResult, ComponentStatus, build_client_with_headers};
use super::json_api::get_config_str;

#[derive(Deserialize)]
struct ProductsResponse {
    products: Vec<Product>,
}

#[derive(Deserialize)]
struct Product {
    title: String,
    id: String,
}

#[derive(Deserialize)]
struct Incident {
    #[serde(default)]
    affected_products: Vec<AffectedProduct>,
    #[serde(default)]
    currently_affected_locations: Vec<Location>,
    status_impact: Option<String>,
    end: Option<String>,
    external_desc: Option<String>,
}

#[derive(Deserialize)]
struct AffectedProduct {
    id: String,
}

#[derive(Deserialize)]
struct Location {
    id: String,
}

/// Runs the Google status adapter for the given watch configuration.
pub async fn run(watch: &Watch) -> Result<AdapterResult, AdapterError> {
    let base_url = watch
        .url
        .as_deref()
        .ok_or_else(|| AdapterError::ParseError("google adapter requires a url".to_string()))?;

    let base_url = base_url.trim_end_matches('/');
    let products_url = format!("{}/products.json", base_url);
    let incidents_url = format!("{}/incidents.json", base_url);

    let client = build_client_with_headers(&watch.headers)?;

    let (products_resp, incidents_resp) = tokio::join!(
        client.get(&products_url).send(),
        client.get(&incidents_url).send(),
    );

    let products: ProductsResponse = products_resp
        .map_err(|e| AdapterError::HttpError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let incidents: Vec<Incident> = incidents_resp
        .map_err(|e| AdapterError::HttpError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AdapterError::ParseError(e.to_string()))?;

    let config = watch.adapter_config.as_ref();
    let product_id = config.and_then(|c| get_config_str(c, "product_id"));
    let location_id = config.and_then(|c| get_config_str(c, "location_id"));

    let filtered_products: Vec<&Product> = match &product_id {
        Some(id) => products.products.iter().filter(|p| p.id == *id).collect(),
        None => products.products.iter().collect(),
    };

    let active_incidents: Vec<&Incident> = incidents
        .iter()
        .filter(|i| i.end.is_none())
        .filter(|i| match &product_id {
            Some(id) => i.affected_products.iter().any(|ap| ap.id == *id),
            None => true,
        })
        .filter(|i| match &location_id {
            Some(lid) => {
                i.currently_affected_locations.iter().any(|l| l.id == *lid)
                    || i.currently_affected_locations.iter().any(|l| l.id == "global")
            }
            None => true,
        })
        .collect();

    let worst_status = aggregate_worst(&active_incidents);

    let description = if active_incidents.is_empty() {
        "All services operational".to_string()
    } else {
        active_incidents
            .first()
            .and_then(|i| i.external_desc.clone())
            .unwrap_or_else(|| format!("{} active incident(s)", active_incidents.len()))
    };

    let display_products: Vec<Product> = filtered_products
        .iter()
        .map(|p| Product { title: p.title.clone(), id: p.id.clone() })
        .collect();
    let components = build_components(&display_products, &active_incidents);

    Ok(AdapterResult {
        status: worst_status,
        description,
        data: serde_json::Value::Null,
        components,
    })
}

/// Determines the worst status across all active incidents.
fn aggregate_worst(incidents: &[&Incident]) -> WatchStatus {
    if incidents.is_empty() {
        return WatchStatus::Success;
    }

    let mut worst = WatchStatus::Success;
    for incident in incidents {
        let status = match incident.status_impact.as_deref() {
            Some("SERVICE_OUTAGE") => WatchStatus::Error,
            Some("SERVICE_DISRUPTION") => WatchStatus::Warning,
            Some("SERVICE_INFORMATION") => WatchStatus::Maintenance,
            _ => WatchStatus::Unknown,
        };
        worst = pick_worse(worst, status);
    }
    worst
}

/// Returns the more severe of two statuses.
fn pick_worse(a: WatchStatus, b: WatchStatus) -> WatchStatus {
    let rank = |s: &WatchStatus| match s {
        WatchStatus::Success => 0,
        WatchStatus::Maintenance => 1,
        WatchStatus::Unknown => 2,
        WatchStatus::Warning => 3,
        WatchStatus::Error => 4,
    };
    if rank(&b) > rank(&a) { b } else { a }
}

/// Builds one component per product, marking affected ones with the incident status.
fn build_components(products: &[Product], active_incidents: &[&Incident]) -> Vec<ComponentStatus> {
    products
        .iter()
        .map(|product| {
            let incident = active_incidents.iter().find(|i| {
                i.affected_products.iter().any(|ap| ap.id == product.id)
            });

            let (status, desc) = match incident {
                Some(i) => {
                    let s = match i.status_impact.as_deref() {
                        Some("SERVICE_OUTAGE") => WatchStatus::Error,
                        Some("SERVICE_DISRUPTION") => WatchStatus::Warning,
                        Some("SERVICE_INFORMATION") => WatchStatus::Maintenance,
                        _ => WatchStatus::Unknown,
                    };
                    let d = i.external_desc.clone().unwrap_or_else(|| "affected".to_string());
                    (s, d)
                }
                None => (WatchStatus::Success, "operational".to_string()),
            };

            ComponentStatus {
                name: product.title.clone(),
                status,
                description: desc,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_incident(impact: &str, end: Option<&str>, product_ids: &[&str]) -> Incident {
        Incident {
            status_impact: Some(impact.to_string()),
            end: end.map(|s| s.to_string()),
            external_desc: Some(format!("{} incident", impact)),
            affected_products: product_ids
                .iter()
                .map(|id| AffectedProduct { id: id.to_string() })
                .collect(),
            currently_affected_locations: vec![],
        }
    }

    #[test]
    fn aggregate_worst_empty_is_success() {
        assert_eq!(aggregate_worst(&[]), WatchStatus::Success);
    }

    #[test]
    fn aggregate_worst_outage_wins() {
        let a = make_incident("SERVICE_INFORMATION", None, &[]);
        let b = make_incident("SERVICE_OUTAGE", None, &[]);
        assert_eq!(aggregate_worst(&[&a, &b]), WatchStatus::Error);
    }

    #[test]
    fn aggregate_worst_disruption_is_warning() {
        let a = make_incident("SERVICE_DISRUPTION", None, &[]);
        assert_eq!(aggregate_worst(&[&a]), WatchStatus::Warning);
    }

    #[test]
    fn build_components_marks_affected_products() {
        let products = vec![
            Product { title: "Gmail".to_string(), id: "gmail-id".to_string() },
            Product { title: "Drive".to_string(), id: "drive-id".to_string() },
            Product { title: "Meet".to_string(), id: "meet-id".to_string() },
        ];
        let incident = make_incident("SERVICE_DISRUPTION", None, &["gmail-id"]);
        let active = vec![&incident];

        let components = build_components(&products, &active);

        assert_eq!(components.len(), 3);
        assert_eq!(components[0].name, "Gmail");
        assert_eq!(components[0].status, WatchStatus::Warning);
        assert_eq!(components[1].name, "Drive");
        assert_eq!(components[1].status, WatchStatus::Success);
        assert_eq!(components[2].name, "Meet");
        assert_eq!(components[2].status, WatchStatus::Success);
    }

    #[test]
    fn build_components_all_operational_when_no_incidents() {
        let products = vec![
            Product { title: "Gmail".to_string(), id: "gmail-id".to_string() },
        ];
        let components = build_components(&products, &[]);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].status, WatchStatus::Success);
        assert_eq!(components[0].description, "operational");
    }
}
