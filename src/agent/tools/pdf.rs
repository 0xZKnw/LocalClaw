//! PDF tools - Create, Read, Modify, Merge PDFs
//!
//! Provides PDF manipulation capabilities using lopdf and printpdf.

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;

use crate::agent::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// PdfReadTool - Extract text from PDF
// ============================================================================

pub struct PdfReadTool;

#[async_trait]
impl Tool for PdfReadTool {
    fn name(&self) -> &str {
        "pdf_read"
    }

    fn description(&self) -> &str {
        "Lire et extraire le texte d'un fichier PDF."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Chemin vers le fichier PDF à lire"
                },
                "pages": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Numéros des pages à extraire (optionnel, toutes par défaut)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;
        
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "Le fichier '{}' n'existe pas", path_str
            )));
        }

        // Use pdf-extract for better text extraction (handles more PDF formats)
        let pages_text = pdf_extract::extract_text_by_pages(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur extraction PDF: {}", e))
        })?;

        let total_pages = pages_text.len();
        
        // Filter requested pages if specified
        let page_filter: Option<Vec<usize>> = if let Some(pages) = params["pages"].as_array() {
            Some(pages.iter()
                .filter_map(|v| v.as_u64().map(|n| (n as usize).saturating_sub(1))) // Convert 1-indexed to 0-indexed
                .collect())
        } else {
            None
        };

        let mut extracted_text = String::new();
        let mut page_texts: Vec<Value> = Vec::new();
        
        // Limit extraction to prevent context saturation
        const MAX_CHARS: usize = 8000;  // ~2000 tokens
        let mut truncated_at_page: Option<usize> = None;

        for (idx, text) in pages_text.iter().enumerate() {
            // Skip if not in filter
            if let Some(ref filter) = page_filter {
                if !filter.contains(&idx) {
                    continue;
                }
            }
            
            // Check if we've hit the limit
            if extracted_text.len() > MAX_CHARS {
                truncated_at_page = Some(idx);
                break;
            }
            
            let page_num = idx + 1; // 1-indexed for display
            let trimmed = text.trim();
            
            if !trimmed.is_empty() {
                extracted_text.push_str(&format!("--- Page {} ---\n{}\n\n", page_num, trimmed));
                page_texts.push(serde_json::json!({
                    "page": page_num,
                    "text": trimmed
                }));
            }
        }
        
        // Add truncation notice
        if let Some(page) = truncated_at_page {
            extracted_text.push_str(&format!(
                "\n[... {} pages restantes tronquées pour économiser le contexte. Utilisez le paramètre 'pages' pour des pages spécifiques.]\n",
                total_pages - page
            ));
        }

        // Fallback message if no text found
        if extracted_text.is_empty() {
            extracted_text = "(Aucun texte extractible - le PDF peut contenir des images ou être scanné)".to_string();
        }

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": path_str,
                "total_pages": total_pages,
                "extracted_pages": page_texts.len(),
                "pages": page_texts,
                "text": extracted_text
            }),
            message: format!(
                "PDF lu: {} pages avec texte sur {} total",
                page_texts.len(), total_pages
            ),
        })
    }
}


// ============================================================================
// PdfCreateTool - Create a new PDF with text content
// ============================================================================

pub struct PdfCreateTool;

#[async_trait]
impl Tool for PdfCreateTool {
    fn name(&self) -> &str {
        "pdf_create"
    }

    fn description(&self) -> &str {
        "Créer un nouveau fichier PDF avec du contenu texte."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Chemin où sauvegarder le PDF"
                },
                "title": {
                    "type": "string",
                    "description": "Titre du document (optionnel)"
                },
                "content": {
                    "type": "string",
                    "description": "Contenu texte du PDF"
                },
                "font_size": {
                    "type": "number",
                    "description": "Taille de police (défaut: 12)"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        use printpdf::*;

        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;
        
        let content = params["content"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("content is required".into()))?;
        
        let title = params["title"].as_str().unwrap_or("Document");
        let font_size = params["font_size"].as_f64().unwrap_or(12.0) as f32;

        // Create PDF document
        let (doc, page1, layer1) = PdfDocument::new(
            title,
            Mm(210.0),  // A4 width
            Mm(297.0),  // A4 height
            "Layer 1"
        );

        let current_layer = doc.get_page(page1).get_layer(layer1);
        
        // Use built-in font
        let font = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur police: {}", e))
        })?;

        // Add content - split into lines (collect as owned strings)
        let lines: Vec<String> = content.lines().map(String::from).collect();
        let line_count = lines.len();
        let line_height = font_size * 1.5;
        let mut y_position = 280.0; // Start from top
        let x_position = 20.0;

        for line in &lines {
            if y_position < 20.0 {
                // Would need to add new page - for now, stop
                break;
            }
            
            current_layer.use_text(
                line,
                font_size,
                Mm(x_position),
                Mm(y_position),
                &font
            );
            
            y_position -= line_height;
        }

        // Save PDF
        let path = PathBuf::from(path_str);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ToolError::ExecutionFailed(format!("Erreur création dossier: {}", e))
                })?;
            }
        }

        let file = std::fs::File::create(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur création fichier: {}", e))
        })?;
        
        let mut buf_writer = std::io::BufWriter::new(file);
        doc.save(&mut buf_writer).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur sauvegarde PDF: {}", e))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": path_str,
                "title": title,
                "lines": line_count,
                "size_bytes": std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            }),
            message: format!("PDF créé: {} ({} lignes)", path_str, line_count),
        })
    }
}

// ============================================================================
// PdfAddPageTool - Add a page to existing PDF
// ============================================================================

pub struct PdfAddPageTool;

#[async_trait]
impl Tool for PdfAddPageTool {
    fn name(&self) -> &str {
        "pdf_add_page"
    }

    fn description(&self) -> &str {
        "Ajouter une page avec du texte à un PDF existant."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Chemin vers le PDF à modifier"
                },
                "content": {
                    "type": "string",
                    "description": "Contenu texte de la nouvelle page"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let path_str = params["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("path is required".into()))?;
        
        let content = params["content"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("content is required".into()))?;

        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "Le fichier '{}' n'existe pas", path_str
            )));
        }

        // Load existing PDF
        let mut doc = lopdf::Document::load(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur lecture PDF: {}", e))
        })?;

        let pages_before = doc.get_pages().len();

        // Create a simple page with text content
        // This is simplified - full implementation would be more complex
        let page_content = format!(
            "BT\n/F1 12 Tf\n50 750 Td\n({}) Tj\nET",
            content.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
        );

        use lopdf::{Object, Dictionary, Stream};
        
        // Create content stream
        let content_id = doc.add_object(Stream::new(
            Dictionary::new(),
            page_content.into_bytes()
        ));

        // Create page dictionary
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".to_vec()));
        page_dict.set("MediaBox", Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(595),  // A4 width in points
            Object::Integer(842),  // A4 height in points
        ]));
        page_dict.set("Contents", Object::Reference(content_id));
        
        // Add font reference (basic)
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        let font_id = doc.add_object(font_dict);
        
        let mut fonts = Dictionary::new();
        fonts.set("F1", Object::Reference(font_id));
        
        let mut resources = Dictionary::new();
        resources.set("Font", Object::Dictionary(fonts));
        page_dict.set("Resources", Object::Dictionary(resources));

        let page_id = doc.add_object(page_dict);

        // Add page to pages tree
        if let Ok(pages_id) = doc.catalog().and_then(|c| c.get(b"Pages")).and_then(|p| p.as_reference()) {
            if let Ok(pages) = doc.get_object_mut(pages_id) {
                if let Object::Dictionary(ref mut pages_dict) = pages {
                    if let Ok(kids) = pages_dict.get_mut(b"Kids") {
                        if let Object::Array(ref mut kids_array) = kids {
                            kids_array.push(Object::Reference(page_id));
                        }
                    }
                    if let Ok(count) = pages_dict.get_mut(b"Count") {
                        if let Object::Integer(ref mut c) = count {
                            *c += 1;
                        }
                    }
                }
            }
        }

        // Save modified PDF
        doc.save(&path).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur sauvegarde PDF: {}", e))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "path": path_str,
                "pages_before": pages_before,
                "pages_after": pages_before + 1
            }),
            message: format!("Page ajoutée au PDF: {} pages maintenant", pages_before + 1),
        })
    }
}

// ============================================================================
// PdfMergeTool - Merge multiple PDFs
// ============================================================================

pub struct PdfMergeTool;

#[async_trait]
impl Tool for PdfMergeTool {
    fn name(&self) -> &str {
        "pdf_merge"
    }

    fn description(&self) -> &str {
        "Fusionner plusieurs fichiers PDF en un seul."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input_files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Liste des chemins des PDFs à fusionner"
                },
                "output_path": {
                    "type": "string",
                    "description": "Chemin du PDF de sortie"
                }
            },
            "required": ["input_files", "output_path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError> {
        let input_files: Vec<String> = params["input_files"]
            .as_array()
            .ok_or_else(|| ToolError::InvalidParameters("input_files is required".into()))?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        
        let output_path = params["output_path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameters("output_path is required".into()))?;

        if input_files.len() < 2 {
            return Err(ToolError::InvalidParameters(
                "Il faut au moins 2 fichiers à fusionner".into()
            ));
        }

        // Verify all input files exist
        for file in &input_files {
            if !PathBuf::from(file).exists() {
                return Err(ToolError::ExecutionFailed(format!(
                    "Le fichier '{}' n'existe pas", file
                )));
            }
        }

        // Load first document as base
        let mut merged_doc = lopdf::Document::load(&input_files[0]).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur lecture {}: {}", input_files[0], e))
        })?;

        let mut total_pages = merged_doc.get_pages().len();

        // Merge other documents
        for input_file in input_files.iter().skip(1) {
            let other_doc = lopdf::Document::load(input_file).map_err(|e| {
                ToolError::ExecutionFailed(format!("Erreur lecture {}: {}", input_file, e))
            })?;
            
            let other_pages = other_doc.get_pages().len();
            
            // Note: lopdf doesn't have merge_pages built-in
            // Return an error - users should use pdf_create instead
            return Err(ToolError::ExecutionFailed(
                "La fusion PDF n'est pas encore supportée. Utilisez pdf_create pour créer de nouveaux PDFs.".into()
            ));
        }

        // Save merged document
        let output = PathBuf::from(output_path);
        if let Some(parent) = output.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ToolError::ExecutionFailed(format!("Erreur création dossier: {}", e))
                })?;
            }
        }

        merged_doc.save(&output).map_err(|e| {
            ToolError::ExecutionFailed(format!("Erreur sauvegarde: {}", e))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "output_path": output_path,
                "input_files": input_files,
                "files_merged": input_files.len(),
                "total_pages": total_pages,
                "size_bytes": std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
            }),
            message: format!(
                "PDF fusionné: {} fichiers -> {} ({} pages)",
                input_files.len(), output_path, total_pages
            ),
        })
    }
}
