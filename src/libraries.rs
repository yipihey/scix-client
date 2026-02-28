//! SciX personal library management endpoints.

use crate::client::SciXClient;
use crate::error::{SciXError, Result};
use crate::types::{Library, LibraryDetail};

impl SciXClient {
    /// List all libraries for the authenticated user.
    pub async fn list_libraries(&self) -> Result<Vec<Library>> {
        let body = self.get("/biblib/libraries", &[]).await?;
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| SciXError::Parse(format!("Invalid libraries response: {}", e)))?;

        let libraries = parsed["libraries"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|lib| {
                Some(Library {
                    id: lib["id"].as_str()?.to_string(),
                    name: lib["name"].as_str().unwrap_or("").to_string(),
                    description: lib["description"].as_str().unwrap_or("").to_string(),
                    num_documents: lib["num_documents"].as_u64().unwrap_or(0) as u32,
                    public: lib["public"].as_bool().unwrap_or(false),
                    owner: lib["owner"].as_str().unwrap_or("").to_string(),
                    date_created: lib["date_created"].as_str().unwrap_or("").to_string(),
                    date_last_modified: lib["date_last_modified"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect();

        Ok(libraries)
    }

    /// Get a library with its documents.
    pub async fn get_library(&self, id: &str) -> Result<LibraryDetail> {
        let body = self
            .get(&format!("/biblib/libraries/{}", id), &[])
            .await?;
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| SciXError::Parse(format!("Invalid library response: {}", e)))?;

        let metadata_val = &parsed["metadata"];
        let metadata = Library {
            id: id.to_string(),
            name: metadata_val["name"].as_str().unwrap_or("").to_string(),
            description: metadata_val["description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            num_documents: metadata_val["num_documents"].as_u64().unwrap_or(0) as u32,
            public: metadata_val["public"].as_bool().unwrap_or(false),
            owner: metadata_val["owner"].as_str().unwrap_or("").to_string(),
            date_created: metadata_val["date_created"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            date_last_modified: metadata_val["date_last_modified"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        };

        let documents = parsed["documents"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|d| d.as_str().map(String::from))
            .collect();

        Ok(LibraryDetail {
            metadata,
            documents,
        })
    }

    /// Create a new library.
    pub async fn create_library(
        &self,
        name: &str,
        description: &str,
        public: bool,
        bibcodes: Option<&[&str]>,
    ) -> Result<Library> {
        let mut body = serde_json::json!({
            "name": name,
            "description": description,
            "public": public,
        });

        if let Some(codes) = bibcodes {
            body["bibcode"] = serde_json::json!(codes);
        }

        let response_body = self.post_json("/biblib/libraries", &body).await?;
        let parsed: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid create library response: {}", e)))?;

        Ok(Library {
            id: parsed["id"].as_str().unwrap_or("").to_string(),
            name: name.to_string(),
            description: description.to_string(),
            num_documents: bibcodes.map(|b| b.len() as u32).unwrap_or(0),
            public,
            owner: String::new(),
            date_created: String::new(),
            date_last_modified: String::new(),
        })
    }

    /// Edit a library's metadata.
    pub async fn edit_library(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        public: Option<bool>,
    ) -> Result<()> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".to_string(), serde_json::json!(n));
        }
        if let Some(d) = description {
            body.insert("description".to_string(), serde_json::json!(d));
        }
        if let Some(p) = public {
            body.insert("public".to_string(), serde_json::json!(p));
        }

        self.put_json(
            &format!("/biblib/documents/{}", id),
            &serde_json::Value::Object(body),
        )
        .await?;
        Ok(())
    }

    /// Delete a library.
    pub async fn delete_library(&self, id: &str) -> Result<()> {
        self.delete(&format!("/biblib/documents/{}", id)).await?;
        Ok(())
    }

    /// Add documents (bibcodes) to a library.
    pub async fn add_documents(&self, library_id: &str, bibcodes: &[&str]) -> Result<()> {
        let body = serde_json::json!({
            "bibcode": bibcodes,
            "action": "add",
        });
        self.post_json(&format!("/biblib/documents/{}", library_id), &body)
            .await?;
        Ok(())
    }

    /// Remove documents (bibcodes) from a library.
    pub async fn remove_documents(&self, library_id: &str, bibcodes: &[&str]) -> Result<()> {
        let body = serde_json::json!({
            "bibcode": bibcodes,
            "action": "remove",
        });
        self.post_json(&format!("/biblib/documents/{}", library_id), &body)
            .await?;
        Ok(())
    }
}
