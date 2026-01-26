use compute_core::schemas::ProjectPageQuery;
use http_contracts::pagination::schema::Pagination;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ProjectPageWithPaginationQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    #[serde(flatten)]
    pub project_page_query: ProjectPageQuery,
}
