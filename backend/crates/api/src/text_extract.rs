//! Text extraction from various document formats
//!
//! Supports: PDF, DOCX, XLSX, PPTX, and plain text formats

use std::io::{Cursor, Read};
use tracing::warn;

/// Error type for text extraction failures
#[derive(Debug)]
pub enum ExtractError {
    UnsupportedFormat(String),
    PdfError(String),
    OfficeError(String),
    IoError(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
            ExtractError::PdfError(msg) => write!(f, "PDF extraction error: {}", msg),
            ExtractError::OfficeError(msg) => write!(f, "Office document error: {}", msg),
            ExtractError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for ExtractError {}

/// Extract text content from a file based on its MIME type
pub fn extract_text(bytes: &[u8], mime_type: &str) -> Result<String, ExtractError> {
    match mime_type {
        // Plain text formats - direct UTF-8 conversion
        "text/plain" 
        | "text/markdown" 
        | "text/csv" 
        | "text/html" 
        | "text/xml"
        | "application/json"
        | "application/xml"
        | "text/x-python"
        | "text/x-java"
        | "text/javascript"
        | "application/javascript"
        | "text/css"
        | "text/x-rust"
        | "text/x-c"
        | "text/x-c++" => {
            Ok(String::from_utf8_lossy(bytes).to_string())
        }
        
        // PDF
        "application/pdf" => extract_pdf(bytes),
        
        // Word documents (.docx)
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            extract_docx(bytes)
        }
        
        // Excel spreadsheets (.xlsx)
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            extract_xlsx(bytes)
        }
        
        // PowerPoint presentations (.pptx)
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            extract_pptx(bytes)
        }
        
        // Legacy Office formats
        "application/msword" => {
            // .doc files are complex binary format, not easily parsed
            Err(ExtractError::UnsupportedFormat(
                "Legacy .doc format not supported. Please convert to .docx".to_string()
            ))
        }
        "application/vnd.ms-excel" => {
            extract_xls(bytes)
        }
        
        _ => Err(ExtractError::UnsupportedFormat(format!(
            "Cannot extract text from: {}",
            mime_type
        ))),
    }
}

/// Extract text from PDF using pdf-extract
fn extract_pdf(bytes: &[u8]) -> Result<String, ExtractError> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| {
            warn!("PDF extraction failed: {:?}", e);
            ExtractError::PdfError(format!("Failed to extract text from PDF: {}", e))
        })
}

/// Extract text from DOCX (Office Open XML Word document)
fn extract_docx(bytes: &[u8]) -> Result<String, ExtractError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| ExtractError::OfficeError(format!("Invalid DOCX file: {}", e)))?;
    
    // DOCX stores content in word/document.xml
    let mut document_xml = match archive.by_name("word/document.xml") {
        Ok(file) => file,
        Err(_) => return Err(ExtractError::OfficeError("No document.xml found in DOCX".to_string())),
    };
    
    let mut xml_content = String::new();
    document_xml.read_to_string(&mut xml_content)
        .map_err(|e| ExtractError::IoError(e.to_string()))?;
    
    // Parse XML and extract text from <w:t> tags
    Ok(extract_text_from_office_xml(&xml_content))
}

/// Extract text from XLSX (Excel spreadsheet)
fn extract_xlsx(bytes: &[u8]) -> Result<String, ExtractError> {
    use calamine::{Reader, Xlsx};
    
    let cursor = Cursor::new(bytes);
    let mut workbook: Xlsx<_> = Xlsx::new(cursor)
        .map_err(|e| ExtractError::OfficeError(format!("Invalid XLSX file: {}", e)))?;
    
    let mut text_parts = Vec::new();
    
    // Get sheet names first
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    
    for sheet_name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            text_parts.push(format!("=== Sheet: {} ===", sheet_name));
            
            for row in range.rows() {
                let row_text: Vec<String> = row.iter()
                    .map(|cell| cell.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if !row_text.is_empty() {
                    text_parts.push(row_text.join("\t"));
                }
            }
        }
    }
    
    Ok(text_parts.join("\n"))
}

/// Extract text from legacy XLS format
fn extract_xls(bytes: &[u8]) -> Result<String, ExtractError> {
    use calamine::{Reader, Xls};
    
    let cursor = Cursor::new(bytes);
    let mut workbook: Xls<_> = Xls::new(cursor)
        .map_err(|e| ExtractError::OfficeError(format!("Invalid XLS file: {}", e)))?;
    
    let mut text_parts = Vec::new();
    
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    
    for sheet_name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            text_parts.push(format!("=== Sheet: {} ===", sheet_name));
            
            for row in range.rows() {
                let row_text: Vec<String> = row.iter()
                    .map(|cell| cell.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if !row_text.is_empty() {
                    text_parts.push(row_text.join("\t"));
                }
            }
        }
    }
    
    Ok(text_parts.join("\n"))
}

/// Extract text from PPTX (PowerPoint presentation)
fn extract_pptx(bytes: &[u8]) -> Result<String, ExtractError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| ExtractError::OfficeError(format!("Invalid PPTX file: {}", e)))?;
    
    let mut text_parts = Vec::new();
    let mut slide_num = 1;
    
    // PPTX stores slides in ppt/slides/slide1.xml, slide2.xml, etc.
    loop {
        let slide_path = format!("ppt/slides/slide{}.xml", slide_num);
        match archive.by_name(&slide_path) {
            Ok(mut file) => {
                let mut xml_content = String::new();
                if file.read_to_string(&mut xml_content).is_ok() {
                    let slide_text = extract_text_from_office_xml(&xml_content);
                    if !slide_text.trim().is_empty() {
                        text_parts.push(format!("--- Slide {} ---", slide_num));
                        text_parts.push(slide_text);
                    }
                }
                slide_num += 1;
            }
            Err(_) => break, // No more slides
        }
    }
    
    if text_parts.is_empty() {
        return Err(ExtractError::OfficeError("No text content found in presentation".to_string()));
    }
    
    Ok(text_parts.join("\n\n"))
}

/// Extract text content from Office Open XML
/// Handles both Word (<w:t>) and PowerPoint (<a:t>) text elements
fn extract_text_from_office_xml(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    
    let mut text_parts = Vec::new();
    let mut in_text_element = false;
    
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                // Match text elements: w:t (Word), a:t (PowerPoint/Drawing)
                if local_name.ends_with(":t") || local_name == "t" {
                    in_text_element = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_element {
                    if let Ok(text) = e.unescape() {
                        let text = text.trim();
                        if !text.is_empty() {
                            text_parts.push(text.to_string());
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let local_name = std::str::from_utf8(name.as_ref()).unwrap_or("");
                if local_name.ends_with(":t") || local_name == "t" {
                    in_text_element = false;
                }
                // Add paragraph break after paragraph elements
                if local_name.ends_with(":p") || local_name == "p" {
                    text_parts.push("\n".to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("XML parsing error: {:?}", e);
                break;
            }
            _ => {}
        }
    }
    
    // Clean up multiple newlines
    text_parts.join(" ")
        .split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Check if a MIME type is supported for text extraction
pub fn is_extractable(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "text/plain"
        | "text/markdown"
        | "text/csv"
        | "text/html"
        | "text/xml"
        | "application/json"
        | "application/xml"
        | "text/x-python"
        | "text/x-java"
        | "text/javascript"
        | "application/javascript"
        | "text/css"
        | "text/x-rust"
        | "text/x-c"
        | "text/x-c++"
        | "application/pdf"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "application/vnd.ms-excel"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plain_text_extraction() {
        let text = b"Hello, World!";
        let result = extract_text(text, "text/plain").unwrap();
        assert_eq!(result, "Hello, World!");
    }
    
    #[test]
    fn test_unsupported_format() {
        let result = extract_text(b"binary", "application/octet-stream");
        assert!(result.is_err());
    }
}

