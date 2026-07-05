use super::Route;

/// Seed utterances per route, English + Vietnamese, used to build the
/// embedding fast path (see router/mod.rs). Small and hand-written for v1 —
/// extend this list if fast-path accuracy needs improvement before reaching
/// for a trained classifier.
pub fn seed_examples() -> Vec<(Route, &'static str)> {
    vec![
        // RAG: questions answerable from ingested documents.
        (Route::Rag, "What is our company's annual leave policy?"),
        (Route::Rag, "How many sick days do employees get per year?"),
        (Route::Rag, "According to the handbook, what is the notice period for resignation?"),
        (Route::Rag, "Chính sách nghỉ phép năm của công ty là gì?"),
        (Route::Rag, "Nhân viên được nghỉ ốm bao nhiêu ngày mỗi năm?"),
        (Route::Rag, "Theo tài liệu nội bộ, quy trình xin nghỉ việc như thế nào?"),
        // Classify: sentiment/topic/intent labeling requests.
        (Route::Classify, "Is this review positive or negative?"),
        (Route::Classify, "Classify this support ticket as urgent, normal, or low priority."),
        (Route::Classify, "What category does this news article belong to?"),
        (Route::Classify, "Đánh giá này là tích cực hay tiêu cực?"),
        (Route::Classify, "Phân loại email này là khẩn cấp hay bình thường."),
        // Extract: pulling structured fields out of text.
        (Route::Extract, "Extract the invoice number, customer name, and total from this receipt."),
        (Route::Extract, "Pull out the order details as JSON: item, quantity, price."),
        (Route::Extract, "Get the name, date, and amount from this document."),
        (Route::Extract, "Trích xuất số hóa đơn, tên khách hàng và tổng tiền từ văn bản này."),
        (Route::Extract, "Lấy ra tên, ngày tháng, và số tiền từ tài liệu này."),
        // Direct chat: general conversation, no retrieval or extraction needed.
        (Route::DirectChat, "Hello, how are you?"),
        (Route::DirectChat, "Tell me a joke."),
        (Route::DirectChat, "What's the capital of France?"),
        (Route::DirectChat, "Xin chào, bạn khỏe không?"),
        (Route::DirectChat, "Kể cho tôi một câu chuyện cười."),
    ]
}
