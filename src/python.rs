//! Python bindings for the scix-client crate.
//!
//! This module provides a synchronous Python API wrapping the async Rust client.
//! All types from `types.rs` are automatically exposed via `#[pyclass(get_all)]`
//! annotations — adding a field to a Rust struct automatically exposes it to Python.
//!
//! Only `SciXClient` (async→sync bridge) and `QueryBuilder` (ownership→mutation bridge)
//! need explicit wrapper types here.

use pyo3::prelude::*;

use crate::client::SciXClient;
use crate::error::SciXError;
use crate::query::QueryBuilder;
use crate::types::*;

/// Convert a SciXError into a Python exception.
fn to_py_err(e: SciXError) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
}

/// Convert a serde_json::Value to a Python object.
fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    pythonize::pythonize(py, value)
        .map(|bound| bound.unbind())
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("JSON conversion error: {}", e))
        })
}

// ---------------------------------------------------------------------------
// Python methods for types defined in types.rs
// ---------------------------------------------------------------------------

#[pymethods]
impl Author {
    /// Parse an author name in ADS format ("Last, First M.").
    #[staticmethod]
    #[pyo3(name = "from_ads_format")]
    fn py_from_ads_format(name: &str) -> Self {
        Author::from_ads_format(name)
    }

    /// Format as "First M. Last" for display.
    #[pyo3(name = "display_name")]
    fn py_display_name(&self) -> String {
        self.display_name()
    }

    /// Format as "Last, First M." for BibTeX.
    #[pyo3(name = "bibtex_name")]
    fn py_bibtex_name(&self) -> String {
        self.bibtex_name()
    }

    fn __repr__(&self) -> String {
        format!("Author('{}')", self.name)
    }

    fn __str__(&self) -> String {
        self.display_name()
    }
}

#[pymethods]
impl Paper {
    fn __repr__(&self) -> String {
        format!("Paper(bibcode='{}', title='{}')", self.bibcode, self.title)
    }
}

#[pymethods]
impl SearchResponse {
    fn __repr__(&self) -> String {
        format!(
            "SearchResponse(num_found={}, papers=[{} returned])",
            self.num_found,
            self.papers.len()
        )
    }

    fn __len__(&self) -> usize {
        self.papers.len()
    }
}

#[pymethods]
impl ExportFormat {
    /// ADS API format string (e.g., "bibtex", "ris").
    #[pyo3(name = "as_api_str")]
    fn py_as_api_str(&self) -> &'static str {
        self.as_api_str()
    }

    /// Parse from string (case-insensitive).
    #[staticmethod]
    #[pyo3(name = "from_str")]
    fn py_from_str(s: &str) -> Option<Self> {
        ExportFormat::from_str_loose(s)
    }

    fn __repr__(&self) -> String {
        format!("ExportFormat.{:?}", self)
    }

    // Note: __str__ is auto-generated from Display impl
}

#[pymethods]
impl PdfLinkType {
    fn __repr__(&self) -> String {
        format!("PdfLinkType.{:?}", self)
    }
}

#[pymethods]
impl Sort {
    /// Create a new sort specification.
    #[new]
    #[pyo3(signature = (field, direction=SortDirection::Desc))]
    fn py_new(field: String, direction: SortDirection) -> Self {
        Sort { field, direction }
    }

    /// Sort by date descending.
    #[staticmethod]
    #[pyo3(name = "date_desc")]
    fn py_date_desc() -> Self {
        Sort::new("date", SortDirection::Desc)
    }

    /// Sort by citation count descending.
    #[staticmethod]
    #[pyo3(name = "citation_count_desc")]
    fn py_citation_count_desc() -> Self {
        Sort::new("citation_count", SortDirection::Desc)
    }

    /// Sort by relevance score descending.
    #[staticmethod]
    #[pyo3(name = "score_desc")]
    fn py_score_desc() -> Self {
        Sort::new("score", SortDirection::Desc)
    }

    fn __repr__(&self) -> String {
        format!("Sort('{}', {:?})", self.field, self.direction)
    }

    // Note: __str__ is auto-generated from Display impl
}

#[pymethods]
impl SortDirection {
    fn __repr__(&self) -> String {
        format!("SortDirection.{:?}", self)
    }
}

#[pymethods]
impl Metrics {
    fn __repr__(&self) -> String {
        "Metrics(...)".to_string()
    }
}

#[pymethods]
impl Library {
    fn __repr__(&self) -> String {
        format!("Library(id='{}', name='{}')", self.id, self.name)
    }
}

#[pymethods]
impl ResolvedReference {
    fn __repr__(&self) -> String {
        format!(
            "ResolvedReference(bibcode={:?})",
            self.bibcode.as_deref().unwrap_or("None")
        )
    }
}

// ---------------------------------------------------------------------------
// PySciXClient — synchronous wrapper around the async SciXClient
// ---------------------------------------------------------------------------

/// SciX (NASA ADS) API client.
///
/// Provides synchronous access to all SciX endpoints.
///
/// Example:
///     client = scix_client.SciXClient()          # reads SCIX_API_TOKEN env var
///     client = scix_client.SciXClient("token")   # explicit token
///     results = client.search("dark matter", rows=10)
#[pyclass(name = "SciXClient")]
struct PySciXClient {
    client: SciXClient,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl PySciXClient {
    /// Create a new SciX client.
    ///
    /// If `token` is None, reads from the `SCIX_API_TOKEN` (or `ADS_API_TOKEN`) environment variable.
    #[new]
    #[pyo3(signature = (token=None))]
    fn new(token: Option<String>) -> PyResult<Self> {
        let client = match token {
            Some(t) => SciXClient::new(t),
            None => SciXClient::from_env().map_err(to_py_err)?,
        };
        let runtime = tokio::runtime::Runtime::new().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to create async runtime: {}",
                e
            ))
        })?;
        Ok(Self { client, runtime })
    }

    /// Set a custom base URL (e.g., for testing).
    fn set_base_url(&mut self, url: String) {
        self.client.base_url = url;
    }

    // -- Search endpoints --

    /// Search SciX using query syntax.
    ///
    /// Example: client.search('author:"Einstein" year:1905', rows=10)
    #[pyo3(signature = (query, rows=10))]
    fn search(&self, query: &str, rows: u32) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(self.client.search(query, rows))
            .map_err(to_py_err)
    }

    /// Search with full control over fields, sort, and pagination.
    #[pyo3(signature = (query, fields="bibcode,title,author,year,pub,abstract,doi,identifier,esources,citation_count,doctype,property", sort=None, rows=10, start=0))]
    fn search_with_options(
        &self,
        query: &str,
        fields: &str,
        sort: Option<&Sort>,
        rows: u32,
        start: u32,
    ) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(
                self.client
                    .search_with_options(query, fields, sort, rows, start),
            )
            .map_err(to_py_err)
    }

    /// Search within a set of known bibcodes.
    #[pyo3(signature = (bibcodes, query=None, fields=None, sort=None, rows=None))]
    fn bigquery(
        &self,
        bibcodes: Vec<String>,
        query: Option<&str>,
        fields: Option<&str>,
        sort: Option<&Sort>,
        rows: Option<u32>,
    ) -> PyResult<SearchResponse> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.bigquery(&refs, query, fields, sort, rows))
            .map_err(to_py_err)
    }

    /// Get papers referenced by the given paper.
    #[pyo3(signature = (bibcode, rows=25))]
    fn references(&self, bibcode: &str, rows: u32) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(self.client.references(bibcode, rows))
            .map_err(to_py_err)
    }

    /// Get papers that cite the given paper.
    #[pyo3(signature = (bibcode, rows=25))]
    fn citations(&self, bibcode: &str, rows: u32) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(self.client.citations(bibcode, rows))
            .map_err(to_py_err)
    }

    /// Get papers similar to the given paper (content-based).
    #[pyo3(signature = (bibcode, rows=10))]
    fn similar(&self, bibcode: &str, rows: u32) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(self.client.similar(bibcode, rows))
            .map_err(to_py_err)
    }

    /// Get co-reads (trending papers read by the same audience).
    #[pyo3(signature = (bibcode, rows=10))]
    fn coreads(&self, bibcode: &str, rows: u32) -> PyResult<SearchResponse> {
        self.runtime
            .block_on(self.client.coreads(bibcode, rows))
            .map_err(to_py_err)
    }

    // -- Export endpoints --

    /// Export papers in the specified citation format.
    #[pyo3(signature = (bibcodes, format=ExportFormat::BibTeX, sort=None))]
    fn export(
        &self,
        bibcodes: Vec<String>,
        format: ExportFormat,
        sort: Option<&Sort>,
    ) -> PyResult<String> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.export(&refs, format, sort))
            .map_err(to_py_err)
    }

    /// Export papers as BibTeX.
    fn export_bibtex(&self, bibcodes: Vec<String>) -> PyResult<String> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.export_bibtex(&refs))
            .map_err(to_py_err)
    }

    // -- Metrics --

    /// Get citation metrics (h-index, g-index, etc.) for papers.
    fn metrics(&self, bibcodes: Vec<String>) -> PyResult<Metrics> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.metrics(&refs))
            .map_err(to_py_err)
    }

    // -- Libraries --

    /// List all libraries for the authenticated user.
    fn list_libraries(&self) -> PyResult<Vec<Library>> {
        self.runtime
            .block_on(self.client.list_libraries())
            .map_err(to_py_err)
    }

    /// Get a library with its documents.
    fn get_library(&self, id: &str) -> PyResult<LibraryDetail> {
        self.runtime
            .block_on(self.client.get_library(id))
            .map_err(to_py_err)
    }

    /// Create a new library.
    #[pyo3(signature = (name, description="", public=false, bibcodes=None))]
    fn create_library(
        &self,
        name: &str,
        description: &str,
        public: bool,
        bibcodes: Option<Vec<String>>,
    ) -> PyResult<Library> {
        let owned_refs: Option<Vec<&str>> = bibcodes
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let refs_slice: Option<&[&str]> = owned_refs.as_deref();
        self.runtime
            .block_on(
                self.client
                    .create_library(name, description, public, refs_slice),
            )
            .map_err(to_py_err)
    }

    /// Edit a library's metadata.
    #[pyo3(signature = (id, name=None, description=None, public=None))]
    fn edit_library(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        public: Option<bool>,
    ) -> PyResult<()> {
        self.runtime
            .block_on(self.client.edit_library(id, name, description, public))
            .map_err(to_py_err)
    }

    /// Delete a library.
    fn delete_library(&self, id: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.delete_library(id))
            .map_err(to_py_err)
    }

    /// Add documents (bibcodes) to a library.
    fn add_documents(&self, library_id: &str, bibcodes: Vec<String>) -> PyResult<()> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.add_documents(library_id, &refs))
            .map_err(to_py_err)
    }

    /// Remove documents (bibcodes) from a library.
    fn remove_documents(&self, library_id: &str, bibcodes: Vec<String>) -> PyResult<()> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.remove_documents(library_id, &refs))
            .map_err(to_py_err)
    }

    /// Get permissions for a library.
    fn get_permissions(&self, py: Python<'_>, library_id: &str) -> PyResult<PyObject> {
        let result = self
            .runtime
            .block_on(self.client.get_permissions(library_id))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    /// Update permissions for a collaborator on a library.
    fn update_permissions(&self, library_id: &str, email: &str, permission: &str) -> PyResult<()> {
        self.runtime
            .block_on(
                self.client
                    .update_permissions(library_id, email, permission),
            )
            .map_err(to_py_err)
    }

    /// Transfer ownership of a library.
    fn transfer_library(&self, library_id: &str, email: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.transfer_library(library_id, email))
            .map_err(to_py_err)
    }

    /// Get a note/annotation on a paper in a library.
    fn get_annotation(&self, library_id: &str, bibcode: &str) -> PyResult<String> {
        self.runtime
            .block_on(self.client.get_annotation(library_id, bibcode))
            .map_err(to_py_err)
    }

    /// Set a note/annotation on a paper in a library.
    fn set_annotation(&self, library_id: &str, bibcode: &str, content: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.set_annotation(library_id, bibcode, content))
            .map_err(to_py_err)
    }

    /// Delete a note/annotation on a paper in a library.
    fn delete_annotation(&self, library_id: &str, bibcode: &str) -> PyResult<()> {
        self.runtime
            .block_on(self.client.delete_annotation(library_id, bibcode))
            .map_err(to_py_err)
    }

    /// Perform a set operation on a library.
    #[pyo3(signature = (library_id, action, source_library_ids=None))]
    fn library_operation(
        &self,
        py: Python<'_>,
        library_id: &str,
        action: &str,
        source_library_ids: Option<Vec<String>>,
    ) -> PyResult<PyObject> {
        let owned_refs: Option<Vec<&str>> = source_library_ids
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let refs_slice: Option<&[&str]> = owned_refs.as_deref();
        let result = self
            .runtime
            .block_on(
                self.client
                    .library_operation(library_id, action, refs_slice),
            )
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    /// Search for papers and add them to a library. Returns count of documents added.
    #[pyo3(signature = (library_id, query, rows=None))]
    fn add_documents_by_query(
        &self,
        library_id: &str,
        query: &str,
        rows: Option<u32>,
    ) -> PyResult<u32> {
        self.runtime
            .block_on(self.client.add_documents_by_query(library_id, query, rows))
            .map_err(to_py_err)
    }

    // -- Reference & object resolution --

    /// Resolve free-text references to ADS bibcodes.
    ///
    /// Example: client.resolve_references(["Einstein 1905 Annalen der Physik 17 891"])
    fn resolve_references(&self, references: Vec<String>) -> PyResult<Vec<ResolvedReference>> {
        let refs: Vec<&str> = references.iter().map(|s| s.as_str()).collect();
        self.runtime
            .block_on(self.client.resolve_references(&refs))
            .map_err(to_py_err)
    }

    /// Resolve astronomical object names to associated bibcodes.
    ///
    /// Returns a dict (raw JSON from ADS).
    fn resolve_objects(&self, py: Python<'_>, objects: Vec<String>) -> PyResult<PyObject> {
        let refs: Vec<&str> = objects.iter().map(|s| s.as_str()).collect();
        let result = self
            .runtime
            .block_on(self.client.resolve_objects(&refs))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    /// Resolve links for a paper (full-text, datasets, citations, references).
    ///
    /// Returns a dict (raw JSON from ADS).
    #[pyo3(signature = (bibcode, link_type=None))]
    fn resolve_links(
        &self,
        py: Python<'_>,
        bibcode: &str,
        link_type: Option<&str>,
    ) -> PyResult<PyObject> {
        let result = self
            .runtime
            .block_on(self.client.resolve_links(bibcode, link_type))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    // -- Network visualization --

    /// Get author collaboration network for papers. Returns a dict.
    fn author_network(&self, py: Python<'_>, bibcodes: Vec<String>) -> PyResult<PyObject> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        let result = self
            .runtime
            .block_on(self.client.author_network(&refs))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    /// Get paper citation/reference network. Returns a dict.
    fn paper_network(&self, py: Python<'_>, bibcodes: Vec<String>) -> PyResult<PyObject> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        let result = self
            .runtime
            .block_on(self.client.paper_network(&refs))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    /// Get co-citation suggestions. Returns a dict.
    fn citation_helper(&self, py: Python<'_>, bibcodes: Vec<String>) -> PyResult<PyObject> {
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        let result = self
            .runtime
            .block_on(self.client.citation_helper(&refs))
            .map_err(to_py_err)?;
        json_to_py(py, &result)
    }

    fn __repr__(&self) -> String {
        format!("SciXClient(base_url='{}')", self.client.base_url)
    }
}

// ---------------------------------------------------------------------------
// PyQueryBuilder — mutation-based wrapper for the ownership-based QueryBuilder
// ---------------------------------------------------------------------------

/// Type-safe SciX query builder.
///
/// Example:
///     q = scix_client.QueryBuilder()
///     q.author("Einstein")
///     q.and_()
///     q.year_range(1905, 1910)
///     results = client.search(q.build(), rows=10)
#[pyclass(name = "QueryBuilder")]
#[derive(Default)]
struct PyQueryBuilder {
    inner: QueryBuilder,
}

#[pymethods]
impl PyQueryBuilder {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Add an author search term.
    fn author(&mut self, name: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.author(name);
    }

    /// Add a first-author search term.
    fn first_author(&mut self, name: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.first_author(name);
    }

    /// Add a title search term.
    fn title(&mut self, text: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.title(text);
    }

    /// Add an abstract search term.
    fn abstract_contains(&mut self, text: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.abstract_contains(text);
    }

    /// Add an exact year filter.
    fn year(&mut self, year: u16) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.year(year);
    }

    /// Add a year range filter (inclusive).
    fn year_range(&mut self, from: u16, to: u16) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.year_range(from, to);
    }

    /// Add a bibcode filter.
    fn bibcode(&mut self, bibcode: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.bibcode(bibcode);
    }

    /// Add a DOI filter.
    fn doi(&mut self, doi: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.doi(doi);
    }

    /// Add an arXiv ID filter.
    fn arxiv(&mut self, arxiv_id: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.arxiv(arxiv_id);
    }

    /// Add an astronomical object filter.
    fn object(&mut self, name: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.object(name);
    }

    /// Add a journal abbreviation filter.
    fn bibstem(&mut self, stem: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.bibstem(stem);
    }

    /// Add a property filter (e.g., "refereed", "openaccess").
    fn property(&mut self, prop: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.property(prop);
    }

    /// Add a doctype filter (e.g., "article", "inproceedings").
    fn doctype(&mut self, dtype: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.doctype(dtype);
    }

    /// Add an ORCID filter.
    fn orcid(&mut self, orcid: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.orcid(orcid);
    }

    /// Add an AND operator.
    #[pyo3(name = "and_")]
    fn py_and(&mut self) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.and();
    }

    /// Add an OR operator.
    #[pyo3(name = "or_")]
    fn py_or(&mut self) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.or();
    }

    /// Add a NOT operator (exclude the following term).
    fn exclude(&mut self) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.exclude();
    }

    /// Add a raw query fragment (no escaping).
    fn raw(&mut self, query: &str) {
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner.raw(query);
    }

    /// Build a citations-of query.
    #[staticmethod]
    fn citations_of(bibcode: &str) -> Self {
        Self {
            inner: QueryBuilder::citations_of(bibcode),
        }
    }

    /// Build a references-of query.
    #[staticmethod]
    fn references_of(bibcode: &str) -> Self {
        Self {
            inner: QueryBuilder::references_of(bibcode),
        }
    }

    /// Build a similar-to query.
    #[staticmethod]
    fn similar_to(bibcode: &str) -> Self {
        Self {
            inner: QueryBuilder::similar_to(bibcode),
        }
    }

    /// Build a trending query.
    #[staticmethod]
    fn trending(bibcode: &str) -> Self {
        Self {
            inner: QueryBuilder::trending(bibcode),
        }
    }

    /// Build the final query string.
    fn build(&self) -> String {
        self.inner.clone().build()
    }

    fn __repr__(&self) -> String {
        format!("QueryBuilder('{}')", self.inner)
    }

    // Note: __str__ is auto-generated from QueryBuilder's Display impl
    // (via the inner field's Display — but QueryBuilder itself isn't Display)
    // Actually, PyQueryBuilder doesn't impl Display, so we provide __str__:
    fn __str__(&self) -> String {
        format!("{}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Python module for the SciX (NASA ADS) API client.
///
/// Usage:
///     import scix_client
///     client = scix_client.SciXClient()
///     results = client.search("dark matter", rows=10)
#[pymodule]
#[pyo3(name = "scix_client")]
pub fn init_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Wrapper types
    m.add_class::<PySciXClient>()?;
    m.add_class::<PyQueryBuilder>()?;

    // Data types (auto-exposed fields via get_all)
    m.add_class::<Paper>()?;
    m.add_class::<Author>()?;
    m.add_class::<PdfLink>()?;
    m.add_class::<PdfLinkType>()?;
    m.add_class::<SearchResponse>()?;
    m.add_class::<ExportFormat>()?;
    m.add_class::<Metrics>()?;
    m.add_class::<BasicStats>()?;
    m.add_class::<BasicStatsEntry>()?;
    m.add_class::<CitationStats>()?;
    m.add_class::<CitationStatsEntry>()?;
    m.add_class::<Indicators>()?;
    m.add_class::<Library>()?;
    m.add_class::<LibraryDetail>()?;
    m.add_class::<ObjectResult>()?;
    m.add_class::<ResolvedReference>()?;
    m.add_class::<Sort>()?;
    m.add_class::<SortDirection>()?;

    Ok(())
}
