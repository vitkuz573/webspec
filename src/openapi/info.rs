use oas3::spec::{Contact, Info, License};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ApiInfo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub contact: Option<ContactInfo>,
    pub license: Option<LicenseInfo>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ContactInfo {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LicenseInfo {
    pub name: Option<String>,
    pub url: Option<String>,
}

pub fn info_from_openapi(info: &Info) -> ApiInfo {
    ApiInfo {
        title: Some(info.title.clone()),
        description: info.description.clone(),
        version: Some(info.version.clone()),
        contact: info.contact.as_ref().map(|c| ContactInfo {
            name: c.name.clone(),
            url: c.url.as_ref().map(|u| u.to_string()),
            email: c.email.clone(),
        }),
        license: info.license.as_ref().map(|l| LicenseInfo {
            name: Some(l.name.clone()),
            url: l.url.as_ref().map(|u| u.to_string()),
        }),
    }
}

pub fn info_to_openapi(title: &str, info: &ApiInfo) -> Info {
    Info {
        title: title.to_string(),
        summary: None,
        description: info.description.clone(),
        terms_of_service: None,
        version: info.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
        contact: info.contact.as_ref().map(|c| Contact {
            name: c.name.clone(),
            url: c.url.as_deref().and_then(|u| u.parse().ok()),
            email: c.email.clone(),
            extensions: oas3::Map::new(),
        }),
        license: info.license.as_ref().and_then(|l| {
            let name = l.name.clone().unwrap_or_default();
            if name.is_empty() {
                None
            } else {
                Some(License {
                    name,
                    url: l.url.as_deref().and_then(|u| u.parse().ok()),
                    identifier: None,
                    extensions: oas3::Map::new(),
                })
            }
        }),
        extensions: oas3::Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_info() {
        let oas_info = Info {
            title: "Example".to_string(),
            summary: None,
            description: Some("Desc".to_string()),
            terms_of_service: None,
            version: "1.0.0".to_string(),
            contact: None,
            license: None,
            extensions: oas3::Map::new(),
        };
        let api_info = info_from_openapi(&oas_info);
        let back = info_to_openapi("Example", &api_info);
        assert_eq!(back.title, "Example");
        assert_eq!(back.description, Some("Desc".to_string()));
    }
}
