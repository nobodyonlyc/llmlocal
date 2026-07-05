#!/usr/bin/env python3
import argparse
import concurrent.futures
import html.parser
import json
import re
import statistics
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


DEFAULT_STORY_URL = "http://14.225.254.182/truyen/qidian/1/1049330853/"
DEFAULT_COMMENT_API = "http://14.225.254.182/io/comment/webComments"
DEFAULT_LLMLocal_URL = "http://127.0.0.1:3000"
VOID_TAGS = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}


class CommentHtmlParser(html.parser.HTMLParser):
    def __init__(self, story_id, source):
        super().__init__(convert_charrefs=True)
        self.story_id = story_id
        self.source = source
        self.comments = []
        self._capture_text = False
        self._text_depth = 0
        self._current_text = []
        self._pending_text = None
        self._pending_time = None
        self._capture_time = False
        self._time_depth = 0
        self._current_time = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        class_names = attrs.get("class", "")

        if tag == "div" and "sec-top" in class_names.split():
            self._capture_text = True
            self._text_depth = 1
            self._current_text = []
            return

        if tag == "br" and self._capture_text:
            self._current_text.append(" ")

        if self._capture_text and tag not in VOID_TAGS:
            self._text_depth += 1

        if tag == "span" and "timeelap" in class_names.split():
            self._capture_time = True
            self._time_depth = 1
            self._current_time = []
            return

        if self._capture_time and tag not in VOID_TAGS:
            self._time_depth += 1

        cmtid = attrs.get("cmtid")
        if tag == "div" and cmtid and self._pending_text:
            self.comments.append(
                {
                    "comment_id": cmtid,
                    "story_id": self.story_id,
                    "source": self.source,
                    "chapter_id": None,
                    "text": normalize_text(self._pending_text),
                    "created_at": normalize_text(self._pending_time or ""),
                    "text_chars": len(normalize_text(self._pending_text)),
                }
            )
            self._pending_text = None
            self._pending_time = None

    def handle_endtag(self, tag):
        if self._capture_text:
            self._text_depth -= 1
            if self._text_depth == 0:
                self._capture_text = False
                self._pending_text = "".join(self._current_text)

        if self._capture_time:
            self._time_depth -= 1
            if self._time_depth == 0:
                self._capture_time = False
                self._pending_time = "".join(self._current_time)

    def handle_data(self, data):
        if self._capture_text:
            self._current_text.append(data)
        if self._capture_time:
            self._current_time.append(data)


def normalize_text(text):
    return re.sub(r"\s+", " ", text).strip()


def parse_comments_html(html_text, story_id, source):
    parser = CommentHtmlParser(story_id=story_id, source=source)
    parser.feed(html_text)
    return [c for c in parser.comments if c["text"]]


def dedupe_comments(comments):
    seen = set()
    result = []
    for comment in comments:
        key = (comment.get("source"), comment.get("story_id"), comment.get("comment_id"))
        if key in seen:
            continue
        seen.add(key)
        result.append(comment)
    return result


def story_urls_from_lines(lines):
    urls = []
    for line in lines:
        line = line.strip()
        if line and not line.startswith("#"):
            urls.append(line)
    return urls


def read_jsonl(path):
    with Path(path).open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                yield json.loads(line)


def write_jsonl(path, rows):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def http_post_form(url, data, referer=None, cookie=None, timeout=30):
    headers = {
        "Content-Type": "application/x-www-form-urlencoded",
        "User-Agent": "Mozilla/5.0",
        "X-Requested-With": "XMLHttpRequest",
    }
    if referer:
        headers["Referer"] = referer
        parsed = urllib.parse.urlparse(referer)
        headers["Origin"] = f"{parsed.scheme}://{parsed.netloc}"
    if cookie:
        headers["Cookie"] = cookie
    encoded = urllib.parse.urlencode(data).encode("utf-8")
    request = urllib.request.Request(url, data=encoded, headers=headers, method="POST")
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read().decode("utf-8", errors="replace")


def http_get(url, timeout=30):
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read().decode("utf-8", errors="replace")


def parse_story_identity(story_url, html_text):
    bookinfo_match = re.search(r"var\s+bookinfo\s*=\s*(\{.*?\});", html_text, re.S)
    if bookinfo_match:
        info = json.loads(bookinfo_match.group(1))
        return {
            "story_id": str(info["id"]),
            "source": str(info["host"]),
            "story_title": normalize_text(info.get("namevi") or info.get("name") or ""),
        }

    hidden_match = re.search(r"id=['\"]hiddenid['\"][^>]*>([^<]+)<", html_text)
    if hidden_match:
        story_id, _chapter, source = hidden_match.group(1).split(";")
        return {"story_id": story_id, "source": source, "story_title": ""}

    parsed = urllib.parse.urlparse(story_url)
    parts = [p for p in parsed.path.split("/") if p]
    if len(parts) >= 4 and parts[0] == "truyen":
        return {"story_id": parts[3], "source": parts[1], "story_title": ""}

    raise ValueError("Could not determine story_id/source from story URL")


def parse_acx_cookie(html_text):
    match = re.search(r"document\.cookie=['\"](_acx=[^;'\" ]+)", html_text)
    return match.group(1) if match else None


def parse_next_start(html_text):
    match = re.search(r"id=['\"]cmtwd['\"][^>]*data-start=['\"](\d+)['\"]", html_text)
    return int(match.group(1)) if match else None


def fetch_comments(args):
    urls = []
    if getattr(args, "story_urls_file", None):
        with args.story_urls_file.open("r", encoding="utf-8") as f:
            urls = story_urls_from_lines(f)
    elif getattr(args, "story_url", None):
        urls = [args.story_url]

    comments = []
    for url in urls:
        if url.startswith("/"):
            url = f"http://14.225.254.182{url}"
        if len(comments) >= args.target:
            break

        try:
            story_html = http_get(url, timeout=args.timeout)
            identity = parse_story_identity(url, story_html)
            cookie = parse_acx_cookie(story_html)
        except Exception as e:
            print(f"Failed to fetch story {url}: {e}", file=sys.stderr)
            continue

        start = 0
        while len(comments) < args.target:
            try:
                body = http_post_form(
                    args.comment_api,
                    {
                        "start": start,
                        "objectid": identity["story_id"],
                        "objecttype": identity["source"],
                    },
                    referer=url,
                    cookie=cookie,
                    timeout=args.timeout,
                )
            except Exception as e:
                print(f"Failed to fetch comments for {url}: {e}", file=sys.stderr)
                break

            page_comments = parse_comments_html(
                body,
                story_id=identity["story_id"],
                source=identity["source"],
            )
            if not page_comments:
                break

            for c in page_comments:
                c["story_title"] = identity["story_title"]

            comments = dedupe_comments(comments + page_comments)
            next_start = parse_next_start(body)
            if next_start is None or next_start <= start:
                start += 10
            else:
                start = next_start

            print(f"fetched={len(comments)} url={url} next_start={start}", file=sys.stderr)
            time.sleep(args.sleep)

    rows = comments[: args.target]
    write_jsonl(args.out, rows)
    print(json.dumps({"comments": len(rows), "out": str(args.out)}, ensure_ascii=False))


def truncate_text(text, max_chars):
    text = text.strip()
    if len(text) <= max_chars:
        return text, False
    return text[:max_chars], True


def classify_comment(llmlocal_url, comment, max_chars, timeout):
    text, truncated = truncate_text(comment["text"], max_chars)
    payload = {
        "comment_id": comment.get("comment_id"),
        "story_id": comment.get("story_id"),
        "chapter_id": comment.get("chapter_id"),
        "text": text,
    }
    request = urllib.request.Request(
        llmlocal_url.rstrip("/") + "/v1/comments/classify",
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=timeout) as response:
        classified = json.loads(response.read().decode("utf-8"))
    latency_ms = round((time.perf_counter() - started) * 1000)
    classified["input_chars"] = len(comment["text"])
    classified["sent_chars"] = len(text)
    classified["truncated"] = truncated
    classified["latency_ms"] = latency_ms
    classified["source_text"] = comment["text"]
    return classified


def classify_comments(args):
    rows = list(read_jsonl(args.input))
    if args.limit:
        rows = rows[: args.limit]

    results = []

    def worker(item):
        index, comment = item
        try:
            result = classify_comment(
                args.llmlocal_url,
                comment,
                max_chars=args.max_chars,
                timeout=args.timeout,
            )
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError) as err:
            result = {
                "comment_id": comment.get("comment_id"),
                "story_id": comment.get("story_id"),
                "error": str(err),
                "source_text": comment.get("text", ""),
            }
        print(f"classified={index}/{len(rows)} id={comment.get('comment_id')}", file=sys.stderr)
        return result

    concurrency = getattr(args, "concurrency", 1)
    if concurrency > 1:
        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
            items = [(i, c) for i, c in enumerate(rows, start=1)]
            results = list(executor.map(worker, items))
    else:
        for index, comment in enumerate(rows, start=1):
            results.append(worker((index, comment)))
            time.sleep(args.sleep)

    write_jsonl(args.out, results)
    print(json.dumps(summarize_results(results) | {"out": str(args.out)}, ensure_ascii=False))


def percentile(values, pct):
    if not values:
        return None
    values = sorted(values)
    index = round((len(values) - 1) * pct)
    return values[index]


def summarize_results(rows):
    summary = {
        "total": len(rows),
        "errors": sum(1 for row in rows if "error" in row),
        "sentiment": {},
        "intent": {},
        "quality_signals": 0,
    }
    latencies = []
    input_lengths = []
    for row in rows:
        if "error" in row:
            continue
        summary["sentiment"][row["sentiment"]] = summary["sentiment"].get(row["sentiment"], 0) + 1
        summary["intent"][row["intent"]] = summary["intent"].get(row["intent"], 0) + 1
        summary["quality_signals"] += 1 if row.get("is_quality_signal") else 0
        latencies.append(row.get("latency_ms", 0))
        input_lengths.append(row.get("input_chars", 0))
    summary["latency_ms"] = {
        "p50": percentile(latencies, 0.50),
        "p90": percentile(latencies, 0.90),
        "max": max(latencies) if latencies else None,
    }
    summary["input_chars"] = {
        "p50": percentile(input_lengths, 0.50),
        "p90": percentile(input_lengths, 0.90),
        "max": max(input_lengths) if input_lengths else None,
    }
    return summary


def summarize_file(args):
    rows = list(read_jsonl(args.input))
    print(json.dumps(summarize_results(rows), ensure_ascii=False, indent=2, sort_keys=True))


def build_parser():
    parser = argparse.ArgumentParser(description="Fetch and classify Sangtacviet story comments.")
    sub = parser.add_subparsers(dest="command", required=True)

    fetch = sub.add_parser("fetch")
    fetch.add_argument("--story-urls-file", type=Path)
    fetch.add_argument("--story-url", default=DEFAULT_STORY_URL)
    fetch.add_argument("--comment-api", default=DEFAULT_COMMENT_API)
    fetch.add_argument("--target", type=int, default=1000)
    fetch.add_argument("--out", type=Path, default=Path("tmp/story-comments.jsonl"))
    fetch.add_argument("--sleep", type=float, default=0.15)
    fetch.add_argument("--timeout", type=float, default=30)
    fetch.set_defaults(func=fetch_comments)

    classify = sub.add_parser("classify")
    classify.add_argument("--input", type=Path, default=Path("tmp/story-comments.jsonl"))
    classify.add_argument("--out", type=Path, default=Path("tmp/story-comment-classifications.jsonl"))
    classify.add_argument("--llmlocal-url", default=DEFAULT_LLMLocal_URL)
    classify.add_argument("--limit", type=int, default=0)
    classify.add_argument("--concurrency", type=int, default=1)
    classify.add_argument("--max-chars", type=int, default=1200)
    classify.add_argument("--sleep", type=float, default=0.0)
    classify.add_argument("--timeout", type=float, default=120)
    classify.set_defaults(func=classify_comments)

    summarize = sub.add_parser("summarize")
    summarize.add_argument("--input", type=Path, default=Path("tmp/story-comment-classifications.jsonl"))
    summarize.set_defaults(func=summarize_file)

    return parser


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main()
