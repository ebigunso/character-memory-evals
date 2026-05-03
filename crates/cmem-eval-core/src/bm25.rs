use std::collections::BTreeMap;

const K1: f64 = 1.2;
const B: f64 = 0.75;

#[derive(Debug, Clone, PartialEq)]
pub struct Bm25Document {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bm25Score {
    pub id: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bm25Index {
    documents: Vec<IndexedDocument>,
    document_frequencies: BTreeMap<String, usize>,
    avg_len: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct IndexedDocument {
    id: String,
    term_frequencies: BTreeMap<String, usize>,
    len: usize,
}

pub fn rank_documents(query: &str, documents: &[Bm25Document]) -> Vec<Bm25Score> {
    Bm25Index::new(documents).rank(query)
}

impl Bm25Index {
    pub fn new(documents: &[Bm25Document]) -> Self {
        let indexed_documents = documents
            .iter()
            .map(|document| {
                let terms = tokenize(&document.text);
                IndexedDocument {
                    id: document.id.clone(),
                    term_frequencies: term_frequencies(&terms),
                    len: terms.len(),
                }
            })
            .collect::<Vec<_>>();
        let avg_len = average_len(&indexed_documents);
        let document_frequencies = document_frequencies(&indexed_documents);
        Self {
            documents: indexed_documents,
            document_frequencies,
            avg_len,
        }
    }

    pub fn rank(&self, query: &str) -> Vec<Bm25Score> {
        let mut scores = self.scores(query);
        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        scores
    }

    pub fn scores(&self, query: &str) -> Vec<Bm25Score> {
        let query_terms = tokenize(query);
        let document_count = self.documents.len() as f64;

        self.documents
            .iter()
            .map(|document| Bm25Score {
                id: document.id.clone(),
                score: bm25_score(
                    &query_terms,
                    document,
                    &self.document_frequencies,
                    document_count,
                    self.avg_len,
                ),
            })
            .collect::<Vec<_>>()
    }
}

pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn term_frequencies(terms: &[String]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for term in terms {
        *out.entry(term.clone()).or_default() += 1;
    }
    out
}

fn bm25_score(
    query_terms: &[String],
    document: &IndexedDocument,
    document_frequencies: &BTreeMap<String, usize>,
    document_count: f64,
    avg_len: f64,
) -> f64 {
    if query_terms.is_empty() || document.len == 0 || document_count == 0.0 {
        return 0.0;
    }

    let document_len = document.len as f64;
    let length_norm = if avg_len == 0.0 {
        1.0
    } else {
        1.0 - B + B * (document_len / avg_len)
    };

    query_terms
        .iter()
        .map(|term| {
            let tf = *document.term_frequencies.get(term).unwrap_or(&0) as f64;
            if tf == 0.0 {
                return 0.0;
            }
            let df = *document_frequencies.get(term).unwrap_or(&0) as f64;
            let idf = ((document_count - df + 0.5) / (df + 0.5) + 1.0).ln();
            idf * ((tf * (K1 + 1.0)) / (tf + K1 * length_norm))
        })
        .sum()
}

fn average_len(documents: &[IndexedDocument]) -> f64 {
    if documents.is_empty() {
        return 0.0;
    }
    documents.iter().map(|document| document.len).sum::<usize>() as f64 / documents.len() as f64
}

fn document_frequencies(documents: &[IndexedDocument]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for document in documents {
        for term in document.term_frequencies.keys() {
            *out.entry(term.clone()).or_default() += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_case_and_punctuation_deterministically() {
        assert_eq!(
            tokenize("Chat-native, chat native! V2"),
            vec!["chat", "native", "chat", "native", "v2"]
        );
    }

    #[test]
    fn ranks_specific_document_above_unrelated_document() {
        let ranked = rank_documents(
            "chat native design",
            &[
                Bm25Document {
                    id: "a".into(),
                    text: "chat native memory design".into(),
                },
                Bm25Document {
                    id: "b".into(),
                    text: "train ticket and travel booking".into(),
                },
            ],
        );

        assert_eq!(ranked[0].id, "a");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn reusable_index_matches_stateless_ranking() {
        let documents = vec![
            Bm25Document {
                id: "a".into(),
                text: "chat native memory design".into(),
            },
            Bm25Document {
                id: "b".into(),
                text: "train ticket and travel booking".into(),
            },
        ];

        assert_eq!(
            Bm25Index::new(&documents).rank("chat native design"),
            rank_documents("chat native design", &documents)
        );
    }

    #[test]
    fn uses_stable_id_tie_breaking_for_zero_scores() {
        let ranked = rank_documents(
            "",
            &[
                Bm25Document {
                    id: "b".into(),
                    text: "beta".into(),
                },
                Bm25Document {
                    id: "a".into(),
                    text: "alpha".into(),
                },
            ],
        );

        assert_eq!(
            ranked
                .iter()
                .map(|score| score.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(ranked[0].score, 0.0);
    }
}
