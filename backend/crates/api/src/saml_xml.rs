//! SAML XML Parsing and Generation
//!
//! Pure Rust SAML XML handling using quick-xml:
//! - AuthnRequest generation (SP-initiated SSO)
//! - SP metadata generation
//! - SAML Response/Assertion parsing

use base64::Engine;
use chrono::{DateTime, Utc};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Cursor;

// ==================== Types ====================

/// Parsed SAML Response
#[derive(Debug)]
pub struct SamlResponse {
    pub id: String,
    pub in_response_to: Option<String>,
    pub issue_instant: Option<String>,
    pub status_code: String,
    pub issuer: Option<String>,
    pub assertion: Option<SamlAssertion>,
    /// Raw XML for signature verification
    pub raw_xml: String,
}

/// Parsed SAML Assertion
#[derive(Debug)]
pub struct SamlAssertion {
    pub id: String,
    pub issuer: String,
    pub name_id: String,
    pub name_id_format: Option<String>,
    pub session_index: Option<String>,
    pub not_before: Option<DateTime<Utc>>,
    pub not_on_or_after: Option<DateTime<Utc>>,
    pub audience: Option<String>,
    pub attributes: HashMap<String, Vec<String>>,
}

// ==================== AuthnRequest Generation ====================

/// Generate a SAML AuthnRequest XML string.
pub fn generate_authn_request(
    request_id: &str,
    sp_entity_id: &str,
    idp_sso_url: &str,
    acs_url: &str,
    nameid_format: &str,
) -> String {
    let issue_instant = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut root = BytesStart::new("samlp:AuthnRequest");
    root.push_attribute(("xmlns:samlp", "urn:oasis:names:tc:SAML:2.0:protocol"));
    root.push_attribute(("xmlns:saml", "urn:oasis:names:tc:SAML:2.0:assertion"));
    root.push_attribute(("ID", request_id));
    root.push_attribute(("Version", "2.0"));
    root.push_attribute(("IssueInstant", issue_instant.as_str()));
    root.push_attribute(("Destination", idp_sso_url));
    root.push_attribute(("AssertionConsumerServiceURL", acs_url));
    root.push_attribute(("ProtocolBinding", "urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"));
    writer.write_event(Event::Start(root)).ok();

    // Issuer
    writer.write_event(Event::Start(BytesStart::new("saml:Issuer"))).ok();
    writer.write_event(Event::Text(BytesText::new(sp_entity_id))).ok();
    writer.write_event(Event::End(BytesEnd::new("saml:Issuer"))).ok();

    // NameIDPolicy
    let mut nid_policy = BytesStart::new("samlp:NameIDPolicy");
    nid_policy.push_attribute(("Format", nameid_format));
    nid_policy.push_attribute(("AllowCreate", "true"));
    writer.write_event(Event::Empty(nid_policy)).ok();

    writer.write_event(Event::End(BytesEnd::new("samlp:AuthnRequest"))).ok();

    String::from_utf8(writer.into_inner().into_inner()).unwrap_or_default()
}

/// Encode an AuthnRequest for HTTP-Redirect binding.
/// Returns the deflated, base64-encoded, URL-encoded string.
pub fn encode_authn_request_redirect(xml: &str) -> String {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml.as_bytes()).unwrap_or_default();
    let compressed = encoder.finish().unwrap_or_default();

    let b64 = base64::engine::general_purpose::STANDARD.encode(&compressed);
    urlencoding::encode(&b64).to_string()
}

/// Encode an AuthnRequest for HTTP-POST binding.
/// Returns the base64-encoded string (for embedding in an HTML form).
pub fn encode_authn_request_post(xml: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(xml.as_bytes())
}

/// Generate an auto-submitting HTML form for HTTP-POST binding.
pub fn generate_post_form(idp_sso_url: &str, saml_request_b64: &str, relay_state: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<body onload="document.forms[0].submit()">
<form method="POST" action="{}">
<input type="hidden" name="SAMLRequest" value="{}" />
<input type="hidden" name="RelayState" value="{}" />
<noscript><button type="submit">Continue to Identity Provider</button></noscript>
</form>
</body>
</html>"#,
        html_escape(idp_sso_url),
        html_escape(saml_request_b64),
        html_escape(relay_state),
    )
}

// ==================== SP Metadata Generation ====================

/// Generate SAML SP metadata XML.
pub fn generate_sp_metadata(
    sp_entity_id: &str,
    acs_url: &str,
    nameid_format: &str,
    signing_cert_pem: Option<&str>,
) -> String {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    let mut root = BytesStart::new("md:EntityDescriptor");
    root.push_attribute(("xmlns:md", "urn:oasis:names:tc:SAML:2.0:metadata"));
    root.push_attribute(("entityID", sp_entity_id));
    writer.write_event(Event::Start(root)).ok();

    // SPSSODescriptor
    let mut sp_sso = BytesStart::new("md:SPSSODescriptor");
    sp_sso.push_attribute(("AuthnRequestsSigned", "false"));
    sp_sso.push_attribute(("WantAssertionsSigned", "true"));
    sp_sso.push_attribute(("protocolSupportEnumeration", "urn:oasis:names:tc:SAML:2.0:protocol"));
    writer.write_event(Event::Start(sp_sso)).ok();

    // Signing certificate (if provided)
    if let Some(cert_pem) = signing_cert_pem {
        let cert_b64 = cert_pem
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");

        let mut kd = BytesStart::new("md:KeyDescriptor");
        kd.push_attribute(("use", "signing"));
        writer.write_event(Event::Start(kd)).ok();

        writer.write_event(Event::Start(BytesStart::new("ds:KeyInfo"))).ok();
        writer.write_event(Event::Start(BytesStart::new("ds:X509Data"))).ok();
        writer.write_event(Event::Start(BytesStart::new("ds:X509Certificate"))).ok();
        writer.write_event(Event::Text(BytesText::new(&cert_b64))).ok();
        writer.write_event(Event::End(BytesEnd::new("ds:X509Certificate"))).ok();
        writer.write_event(Event::End(BytesEnd::new("ds:X509Data"))).ok();
        writer.write_event(Event::End(BytesEnd::new("ds:KeyInfo"))).ok();
        writer.write_event(Event::End(BytesEnd::new("md:KeyDescriptor"))).ok();
    }

    // NameIDFormat
    writer.write_event(Event::Start(BytesStart::new("md:NameIDFormat"))).ok();
    writer.write_event(Event::Text(BytesText::new(nameid_format))).ok();
    writer.write_event(Event::End(BytesEnd::new("md:NameIDFormat"))).ok();

    // AssertionConsumerService
    let mut acs = BytesStart::new("md:AssertionConsumerService");
    acs.push_attribute(("Binding", "urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"));
    acs.push_attribute(("Location", acs_url));
    acs.push_attribute(("index", "0"));
    acs.push_attribute(("isDefault", "true"));
    writer.write_event(Event::Empty(acs)).ok();

    writer.write_event(Event::End(BytesEnd::new("md:SPSSODescriptor"))).ok();
    writer.write_event(Event::End(BytesEnd::new("md:EntityDescriptor"))).ok();

    String::from_utf8(writer.into_inner().into_inner()).unwrap_or_default()
}

// ==================== SAML Response Parsing ====================

/// Parse a base64-encoded SAML Response.
pub fn parse_saml_response(base64_xml: &str) -> Result<SamlResponse, String> {
    let xml_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_xml.replace(char::is_whitespace, ""))
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    let xml = String::from_utf8(xml_bytes)
        .map_err(|e| format!("UTF-8 decode error: {}", e))?;

    parse_saml_response_xml(&xml)
}

/// Parse SAML Response XML string.
fn parse_saml_response_xml(xml: &str) -> Result<SamlResponse, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut response = SamlResponse {
        id: String::new(),
        in_response_to: None,
        issue_instant: None,
        status_code: String::new(),
        issuer: None,
        assertion: None,
        raw_xml: xml.to_string(),
    };

    // Track parsing context
    let mut _in_response = false;
    let mut in_assertion = false;
    let mut in_subject = false;
    let mut _in_conditions = false;
    let mut in_attribute_statement = false;
    let mut _in_authn_statement = false;
    let mut in_name_id = false;
    let mut in_issuer = false;
    let mut in_audience = false;
    let mut in_attribute_value = false;
    let mut issuer_depth = 0; // 0=response level, 1=assertion level

    let mut assertion = SamlAssertion {
        id: String::new(),
        issuer: String::new(),
        name_id: String::new(),
        name_id_format: None,
        session_index: None,
        not_before: None,
        not_on_or_after: None,
        audience: None,
        attributes: HashMap::new(),
    };

    let mut current_attr_name = String::new();
    let mut current_attr_values: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local.as_str() {
                    "Response" => {
                        _in_response = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "ID" => response.id = val,
                                "InResponseTo" => response.in_response_to = Some(val),
                                "IssueInstant" => response.issue_instant = Some(val),
                                _ => {}
                            }
                        }
                    }
                    "Assertion" => {
                        in_assertion = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "ID" {
                                assertion.id = val;
                            }
                        }
                    }
                    "Issuer" => {
                        in_issuer = true;
                        issuer_depth = if in_assertion { 1 } else { 0 };
                    }
                    "StatusCode" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "Value" {
                                response.status_code = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "Subject" => in_subject = true,
                    "NameID" => {
                        in_name_id = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "Format" {
                                assertion.name_id_format = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    "Conditions" => {
                        _in_conditions = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "NotBefore" => {
                                    assertion.not_before = DateTime::parse_from_rfc3339(&val).ok().map(|d| d.with_timezone(&Utc));
                                }
                                "NotOnOrAfter" => {
                                    assertion.not_on_or_after = DateTime::parse_from_rfc3339(&val).ok().map(|d| d.with_timezone(&Utc));
                                }
                                _ => {}
                            }
                        }
                    }
                    "Audience" | "AudienceRestriction" => {
                        if local == "Audience" {
                            in_audience = true;
                        }
                    }
                    "AuthnStatement" => {
                        _in_authn_statement = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "SessionIndex" {
                                assertion.session_index = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    "AttributeStatement" => in_attribute_statement = true,
                    "Attribute" if in_attribute_statement => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "Name" {
                                current_attr_name = String::from_utf8_lossy(&attr.value).to_string();
                                current_attr_values.clear();
                            }
                        }
                    }
                    "AttributeValue" if in_attribute_statement => {
                        in_attribute_value = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local == "StatusCode" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key == "Value" {
                            response.status_code = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_name_id && in_subject {
                    assertion.name_id = text;
                } else if in_issuer {
                    if issuer_depth == 0 {
                        response.issuer = Some(text);
                    } else {
                        assertion.issuer = text;
                    }
                } else if in_audience {
                    assertion.audience = Some(text);
                } else if in_attribute_value {
                    current_attr_values.push(text);
                }
            }
            Ok(Event::End(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local.as_str() {
                    "Response" => _in_response = false,
                    "Assertion" => {
                        in_assertion = false;
                        response.assertion = Some(assertion);
                        // Reset for potential additional assertions (we only care about the first)
                        assertion = SamlAssertion {
                            id: String::new(),
                            issuer: String::new(),
                            name_id: String::new(),
                            name_id_format: None,
                            session_index: None,
                            not_before: None,
                            not_on_or_after: None,
                            audience: None,
                            attributes: HashMap::new(),
                        };
                    }
                    "Subject" => in_subject = false,
                    "NameID" => in_name_id = false,
                    "Issuer" => in_issuer = false,
                    "Conditions" => _in_conditions = false,
                    "Audience" => in_audience = false,
                    "AuthnStatement" => _in_authn_statement = false,
                    "AttributeStatement" => in_attribute_statement = false,
                    "Attribute" if in_attribute_statement => {
                        if !current_attr_name.is_empty() {
                            assertion.attributes.insert(
                                current_attr_name.clone(),
                                current_attr_values.clone(),
                            );
                        }
                        current_attr_name.clear();
                        current_attr_values.clear();
                    }
                    "AttributeValue" => in_attribute_value = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    if response.id.is_empty() {
        return Err("Missing Response ID".to_string());
    }

    Ok(response)
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
