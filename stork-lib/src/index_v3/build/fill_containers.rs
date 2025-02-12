use rust_stemmers::Stemmer;
use std::{collections::BTreeMap, convert::TryInto};

use crate::{
    config::Config,
    index_v3::{
        scores::{PREFIX_SCORE, STEM_SCORE},
        AnnotatedWord, Container, Excerpt, SearchResult, WordListSource,
    },
};

use super::{
    annotated_words_from_string::AnnotatedWordable, intermediate_entry::NormalizedEntry,
    remove_surrounding_punctuation,
};

pub fn fill_containers(
    config: &Config,
    intermediate_entries: &[NormalizedEntry],
    stems: &BTreeMap<String, Vec<String>>,
    containers: &mut BTreeMap<String, Container>,
) {
    for (entry_index, entry) in intermediate_entries.iter().enumerate() {
        let words_in_title: Vec<AnnotatedWord> = entry.title.make_annotated_words();
        let words_in_contents: Vec<AnnotatedWord> = entry.annotated_word_list.word_list.clone();

        let word_lists = vec![
            (WordListSource::Title, words_in_title),
            (WordListSource::Contents, words_in_contents),
        ];

        for (source, word_list) in word_lists {
            for (word_index, annotated_word) in word_list.iter().enumerate() {
                let normalized_word =
                    remove_surrounding_punctuation(&annotated_word.word.to_lowercase());

                const MAX_WORD_LENGTH: usize = 2usize.pow(12);

                // Filter out empty and pathologically long words
                if normalized_word.is_empty() || normalized_word.len() > MAX_WORD_LENGTH {
                    continue;
                }

                fill_container_results_map(
                    config,
                    containers,
                    &normalized_word,
                    word_index,
                    entry_index,
                    annotated_word,
                    source,
                );

                // Step 2B: Fill _other containers'_ aliases maps with the
                // prefixes of this word
                fill_other_containers_alias_maps_with_prefixes(
                    config.input.minimum_indexed_substring_length,
                    config.input.minimum_index_ideographic_substring_length,
                    containers,
                    &normalized_word,
                );

                // Step 2C: Fill _other containers'_ alias maps with the
                // reverse-stems of this word
                fill_other_containers_alias_maps_with_reverse_stems(
                    entry,
                    stems,
                    containers,
                    &normalized_word,
                );
            }
        }
    }
}

fn fill_container_results_map(
    config: &Config,
    containers: &mut BTreeMap<String, Container>,
    normalized_word: &str,
    word_index: usize,
    entry_index: usize,
    annotated_word: &AnnotatedWord,
    source: WordListSource,
) {
    let results_map = &mut containers
        .entry(normalized_word.to_string())
        .or_insert_with(Container::new)
        .results;

    let entry_result: &mut SearchResult = results_map
        .entry(entry_index)
        .or_insert_with(SearchResult::new);

    if config.output.excerpts_per_result > 0 {
        entry_result.excerpts.push(Excerpt {
            word_index,
            source,
            internal_annotations: annotated_word.internal_annotations.clone(),
            fields: annotated_word.fields.clone(),
        });
    }
}

fn fill_other_containers_alias_maps_with_prefixes(
    prefix_length: u8,
    ideograph_prefix_length: u8,
    containers: &mut BTreeMap<String, Container>,
    normalized_word: &str,
) {
    let characters_in_string = normalized_word.chars().count();

    let skip_chars: usize = if string_is_cjk_ideographic(normalized_word.chars()) {
        ideograph_prefix_length as usize
    } else {
        prefix_length as usize
    };

    for (char_count, (char_boundary, _)) in
        normalized_word.char_indices().enumerate().skip(skip_chars)
    {
        let substring = &normalized_word[..char_boundary];

        if !containers.contains_key(substring) {
            containers.insert(substring.to_string(), Container::new());
        }
        let alises_map = &mut containers.get_mut(substring).unwrap().aliases;

        let _alias_score = alises_map
            .entry(normalized_word.to_string())
            .or_insert_with(|| {
                PREFIX_SCORE.saturating_sub(
                    characters_in_string
                        .saturating_sub(char_count)
                        .try_into()
                        .unwrap_or(u8::MAX),
                )
            });
    }
}

fn fill_other_containers_alias_maps_with_reverse_stems(
    entry: &NormalizedEntry,
    stems: &BTreeMap<String, Vec<String>>,
    containers: &mut BTreeMap<String, Container>,
    normalized_word: &str,
) {
    if let Some(stem_algorithm) = entry.stem_algorithm {
        let stem = Stemmer::create(stem_algorithm)
            .stem(normalized_word)
            .to_string();
        if let Some(reverse_stems_vector) = stems.get(&stem) {
            for reverse_stem in reverse_stems_vector {
                if reverse_stem != normalized_word {
                    let _alias_score = containers
                        .entry(reverse_stem.clone())
                        .or_insert_with(Container::new)
                        .aliases
                        .entry(normalized_word.to_string())
                        .or_insert(STEM_SCORE);
                }
            }
        }
    }
}

fn string_is_cjk_ideographic(mut s: impl Iterator<Item = char>) -> bool {
    s.all(char_is_cjk_ideograph)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn char_is_cjk_ideograph(c: char) -> bool {
    // Block ranges sourced from https://en.wikipedia.org/wiki/CJK_Unified_Ideographs#CJK_Unified_Ideographs_blocks
    matches!(c,
        // CJK Unified Ideographs
        '\u{4E00}'..='\u{62FF}' |
        '\u{6300}'..='\u{77FF}' |
        '\u{7800}'..='\u{8CFF}' |
        '\u{8D00}'..='\u{9FFF}' |
        // CJK Unified Ideographs Extension A
        '\u{3400}'..='\u{4DBF}' |
        // CJK Unified Ideographs Extension B
        '\u{20000}'..='\u{215FF}' |
        '\u{21600}'..='\u{230FF}' |
        '\u{23100}'..='\u{245FF}' |
        '\u{24600}'..='\u{260FF}' |
        '\u{26100}'..='\u{275FF}' |
        '\u{27600}'..='\u{290FF}' |
        '\u{29100}'..='\u{2A6DF}' |
        // CJK Unified Ideographs Extension C
        '\u{2A700}'..='\u{2B73F}' |
        // CJK Unified Ideographs Extension D
        '\u{2B740}'..='\u{2B81F}' |
        // CJK Unified Ideographs Extension E
        '\u{2B820}'..='\u{2CEAF}' |
        // CJK Unified Ideographs Extension F
        '\u{2CEB0}'..='\u{2EBEF}' |
        // CJK Unified Ideographs Extension G
        '\u{30000}'..='\u{3134F}' |
        // CJK Compatibility Ideographs
        '\u{F900}'..='\u{FAFF}'
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        config::Config,
        index_v3::{build::intermediate_entry::NormalizedEntry, AnnotatedWordList},
    };
    use std::collections::{BTreeMap, HashMap};

    use super::fill_containers;

    #[test]
    fn container_filling_continues_after_encountering_unnormalizable_word() {
        let intermediate_entry = NormalizedEntry {
            annotated_word_list: AnnotatedWordList { word_list: vec![] },
            title: "10 - Polymorphism".to_string(),
            url: String::new(),
            fields: HashMap::default(),
            stem_algorithm: None,
        };

        let mut containers = BTreeMap::default();

        fill_containers(
            &Config::default(),
            &[intermediate_entry],
            &BTreeMap::default(),
            &mut containers,
        );

        assert!(
            containers.get("polymorphism").is_some(),
            "Containers did not contain `polymorphism`, containers was {:?}",
            containers.keys()
        );
    }
}
