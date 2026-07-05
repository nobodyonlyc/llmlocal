/// Splits text into paragraph-bounded chunks, each roughly `target_chars` long.
/// A char-count heuristic stands in for a token count for v1 — good enough to
/// prove the ingest -> embed -> retrieve pipeline; swap for a tokenizer-aware
/// splitter later if chunk boundaries turn out to matter for retrieval quality.
pub fn chunk_text(text: &str, target_chars: usize) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        if !current.is_empty() && current.len() + para.len() + 2 > target_chars {
            chunks.push(std::mem::take(&mut current));
        }

        if para.len() > target_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            // Oversized single paragraph: split on sentence boundaries.
            for sentence in split_sentences(para) {
                if !current.is_empty() && current.len() + sentence.len() + 1 > target_chars {
                    chunks.push(std::mem::take(&mut current));
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(sentence);
            }
            continue;
        }

        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn split_sentences(text: &str) -> Vec<&str> {
    text.split_inclusive(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}
