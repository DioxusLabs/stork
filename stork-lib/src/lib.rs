#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;

use bytes::Bytes;
use itertools::Itertools;
use search_query::SearchQuery;
use smart_default::SmartDefault;

mod envelope;
mod importance_value;
mod string_utils;

mod index_v4;

pub mod parse_index;
pub mod search_output;
pub mod search_query;
pub mod search_value;

pub type Fields = HashMap<String, String>;

#[cfg(feature = "build")]
pub mod build;

#[cfg(feature = "build")]
pub mod build_config;

#[cfg(feature = "build")]
pub mod build_output;

#[cfg(feature = "build")]
use build::ProgressReporter;

/// Builds an index from a configuration.
#[cfg(feature = "build")]
pub fn build_index(
    config: &build_config::Config,
    progress: impl ProgressReporter,
) -> Result<build_output::BuildSuccessValue, build_output::errors::BuildError> {
    build::build_index(config, progress).map_err(build_output::errors::BuildError::from)
}

/// Given some bytes, this function will try to unwrap it from its envelope and
/// parse it as a search index, returning that index if it's successful.
pub fn parse_bytes_as_index(
    bytes: Bytes,
) -> Result<parse_index::ParsedIndex, parse_index::errors::IndexParseError> {
    parse_index::parse(bytes)
}

/// Some search indexes come in multiple chunks. If you've parsed the primary
/// chunk of an index and you have the bytes of a secondary chunk, you can use
/// this function to add that secondary data to your original index.
///
/// This function will mutate the index you pass in.
pub fn add_sidecar_bytes_to_index(
    index: &mut parse_index::ParsedIndex,
    bytes: Bytes,
) -> Result<(), parse_index::errors::IndexParseError> {
    parse_index::add_sidecar_bytes_to_index(index, bytes)
}

pub fn get_search_values(
    index: &parse_index::ParsedIndex,
    term: &search_query::SearchTerm,
) -> Result<Vec<search_value::SearchValue>, search_output::errors::SearchError> {
    match &index.value {
        parse_index::IndexType::V4Index(v4_index) => {
            index_v4::search::get_search_values(v4_index, term)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SmartDefault)]
pub struct SearchConfig {
    /// The length, in characters, that a rendered excerpt will be. Controls
    /// excerpt merging as well as display length.
    /// Defaults to 150.
    #[default = 150]
    pub excerpt_length: usize,

    /// The maximum number of documents returned in the search results.
    /// Defaults to 10.
    #[default = 10]
    pub number_of_results: usize,

    /// The maximum number of excerpts returned for each document.
    /// Defaults to 5.
    #[default = 5]
    pub number_of_excerpts: usize,
}

pub fn merge_search_values(
    index: &parse_index::ParsedIndex,
    search_values: Vec<Vec<search_value::SearchValue>>,
    config: &SearchConfig,
) -> Result<search_output::SearchOutput, search_output::errors::SearchError> {
    match &index.value {
        parse_index::IndexType::V4Index(v4_index) => {
            let search_values = search_values
                .iter()
                .flatten()
                .filter_map(|sv| sv.v4_value.clone()) // TODO: Throw a user-visible error if there are non-v4 search values
                .collect_vec();

            index_v4::search::render_search_values(v4_index, search_values, config)
        }
    }
}

pub fn search(
    index: &parse_index::ParsedIndex,
    query: &str,
    config: &SearchConfig,
) -> Result<search_output::SearchOutput, search_output::errors::SearchError> {
    let terms = query
        .parse::<SearchQuery>()
        .map_err(|_| search_output::errors::SearchError::NotCompiledWithFeature)?; // TODO: Replace with accurate error

    let search_values = terms
        .items
        .iter()
        .flat_map(|term| get_search_values(index, term))
        .collect_vec();

    merge_search_values(index, search_values, config)
}
