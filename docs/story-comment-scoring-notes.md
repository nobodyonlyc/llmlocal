# Story Comment Scoring Notes

## Goal

Use user comments as a core signal to rank/filter stories by likely quality, without letting long stories dominate only because they have more chapters and therefore more comments.

## Current Metadata

- `author`
- `chapter_id`
- `chapter_number`
- `chapter_title`
- `source`
- `story_id`
- `story_title`
- `url`
- comment count
- read count
- like count
- follow count
- bookmark count
- story brief

## Main Principle

Do not use raw comment count as a direct quality score.

Raw comment count mostly measures activity, popularity, controversy, chapter count, or update pressure. It does not reliably say whether the story is good or bad.

Instead, split scoring into three separate signals:

- `quality_score`: whether valid story-related comments are positive or negative.
- `popularity_score`: whether many users read, like, follow, or bookmark the story.
- `confidence_score`: whether there is enough reliable data to trust the quality score.

## Comment Classification API Structure

To interact with the local LLM for classification:

- **Endpoint**: `POST /v1/comments/classify`
- **Payload Format**:
  ```json
  {
    "comment_id": "string",
    "story_id": "string",
    "chapter_id": "string (optional)",
    "text": "string (max 1200 chars)"
  }
  ```
- **Response Format**:
  ```json
  {
    "comment_id": "string",
    "story_id": "string",
    "sentiment": "positive | negative | neutral | mixed",
    "intent": "story_quality | translation_quality | update_request | spam | social | question",
    "is_quality_signal": true
  }
  ```

Important distinction for intents:

- `story_quality`: useful for judging whether the story is good or bad.
- `translation_quality`: useful, but should not be treated exactly the same as story quality.
- `update_request`, `social`, `question`, `spam`: useful for engagement/noise analysis, but weak quality signals.

Examples:

- "Truyen hay, main thong minh" -> positive + story_quality.
- "Cang ve sau cang nat" -> negative + story_quality.
- "Dich kho doc qua" -> negative + translation_quality.
- "Ra chuong nhanh di" -> neutral + update_request.
- "Tem", "hong", "thanks" -> social or low-value engagement.

## Comment Selection Algorithm (Sampling Strategy)

Evaluating all comments for popular stories (which can have tens of thousands) is too slow and expensive. We need a sampling algorithm to select the most representative comments (e.g., max 100-200 comments per story):

1. **Length Filter**: Ignore very short comments (< 15 characters like "hay", "tem", "hóng") before sending to the LLM, as they rarely contain deep quality signals.
2. **Engagement Priority**: Sort or prioritize comments with high upvotes/likes/replies, as they often represent community consensus.
3. **Temporal/Chapter Distribution**:
   - **Early Chapters (First 10%)**: Sample to check if the premise/hook is good.
   - **Middle Chapters**: Sample to see if the pacing drags.
   - **Latest/Recent Chapters**: Heavily sample recent comments to detect "late-stage collapse" (đầu voi đuôi chuột) - a very common issue in long web novels.
4. **Keyword Heuristics**: Pre-filter or over-sample comments containing strong sentiment keywords (e.g., "não tàn", "rác", "siêu phẩm", "logic", "dịch hay") to ensure critical opinions are captured.

## Normalized Comment Metrics

Use only valid comments for quality scoring:

```text
valid_comment_count = comments excluding spam and very low-value social comments
quality_comment_count = comments with intent = story_quality
comments_per_chapter = valid_comment_count / chapter_count
positive_ratio = positive_story_quality_comments / quality_comment_count
negative_ratio = negative_story_quality_comments / quality_comment_count
```

If chapter-level comment timestamps or chapter ids are available, also calculate recent trend:

```text
recent_positive_ratio = positive ratio from recent chapters/comments
recent_negative_ratio = negative ratio from recent chapters/comments
recent_negative_trend = recent_negative_ratio - overall_negative_ratio
```

This helps catch stories that start well but become bad later.

## Bayesian Smoothing

Avoid ranking a story too highly just because it has only a few positive comments.

Use Bayesian smoothing:

```text
smoothed_positive =
  (positive_quality_comments + prior_positive * prior_weight)
  / (quality_comment_count + prior_weight)
```

Suggested initial values:

```text
prior_positive = 0.60
prior_negative = 0.20
prior_weight = 20
```

Meaning: before trusting a story's comments, assume an average baseline based on about 20 virtual comments. As real comments increase, the actual data dominates.

## Initial Scoring Formula

Start with a simple, explainable formula:

```text
comment_quality_score =
  smoothed_positive
- 0.8 * smoothed_negative
- 0.4 * translation_complaint_ratio
- 0.3 * recent_negative_trend
```

Then combine with metadata:

```text
story_score =
  0.45 * comment_quality_score
+ 0.20 * like_read_score
+ 0.15 * follow_bookmark_score
+ 0.10 * engagement_per_chapter_score
+ 0.10 * brief_relevance_or_genre_score
```

The weights are starting values. They should be tuned after seeing real data.

## Metadata Normalization

Use log normalization for large count fields so popular stories do not overwhelm the score:

```text
normalized_reads = log(1 + read_count)
normalized_likes = log(1 + like_count)
normalized_follows = log(1 + follow_count)
normalized_bookmarks = log(1 + bookmark_count)
```

Useful derived metrics:

```text
like_read_score = log(1 + like_count) / log(1 + read_count)
follow_bookmark_score = combined normalized follow and bookmark count
engagement_per_chapter_score = valid_comment_count / chapter_count
```

## Recommended Filter Types

Expose or use separate ranking modes instead of one universal score:

- `High quality`: high `quality_score`, low negative ratio, medium/high confidence.
- `Reliable pick`: high `quality_score` and high `confidence_score`.
- `Potential hidden gem`: high `quality_score`, lower popularity, lower/medium confidence.
- `Hot but controversial`: high popularity and high mixed/negative activity.
- `Avoid`: high negative ratio, high recent negative trend, or many story-quality complaints.

## Practical Pipeline

1. Group comments by `story_id`.
2. Classify each comment.
3. Remove spam and low-value social comments from quality scoring.
4. Aggregate story-level counts:
   - positive story-quality comments
   - negative story-quality comments
   - neutral/mixed comments
   - translation complaints
   - update requests
   - spam/noise
5. Normalize by chapter count.
6. Apply Bayesian smoothing.
7. Calculate `quality_score`, `popularity_score`, and `confidence_score`.
8. Use different ranking/filter modes depending on the goal.

## Aggregating Results for Final Evaluation

Once the sampled comments are classified, use these methods to aggregate the results and evaluate the story:

1. **Quality Signal Ratio**:
   - Only count comments where `is_quality_signal` is true (or intent is `story_quality`).
   - `Quality Ratio = Positive Quality Comments / Total Quality Comments`.
   
2. **Late-Stage Collapse Penalty**:
   - Compare the negative ratio of the most recent 20% of chapters against the first 80%.
   - If `recent_negative_ratio >> overall_negative_ratio`, apply a penalty multiplier to the final score. This directly addresses the common problem of stories dropping in quality later on.

3. **Bayesian Confidence Scoring**:
   - If a story has too few quality comments (e.g., < 10), the confidence is low.
   - Use Bayesian smoothing to pull the score towards the average baseline until more data is collected.
   - `Smoothed Score = (Positive + Prior_Positive*Weight) / (Total + Prior_Weight)`

4. **Automated Tagging Generation**:
   - Based on aggregated intents, automatically generate warning or praise tags for the story:
     - *High `translation_quality` (Negative)* -> Auto-tag: "Bản dịch tệ/Convert khó đọc"
     - *High `update_request`* -> Auto-tag: "Tác giả ra chương chậm"
     - *High `story_quality` (Positive)* -> Auto-tag: "Cốt truyện hay / Đánh giá cao"

## Key Takeaway

The core signal should be:

```text
Bayesian-smoothed positive/negative story-quality comment ratio,
adjusted by late-stage chapter trends,
and filtered by a smart sampling algorithm to reduce LLM workload.
```

This is much more reliable, efficient, and insightful than using total comment counts.
