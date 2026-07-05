import importlib.util
import pathlib
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "eval_story_comments.py"
spec = importlib.util.spec_from_file_location("eval_story_comments", SCRIPT)
eval_story_comments = importlib.util.module_from_spec(spec)
spec.loader.exec_module(eval_story_comments)


class CommentParserTests(unittest.TestCase):
    def test_parse_comments_extracts_text_and_metadata(self):
        html = """
        <div class="flex">
          <div class="sec-top bg-gray">Truyen hay &amp; cuon<br>main thong minh</div>
          <div cmtid="101"><a href="/user/1/">Reader A</a> -
          <span class="timeelap">Sun Jul 05 2026 10:00:00 GMT+0700</span></div>
        </div>
        <div class="flex">
          <div class="sec-top bg-gray"><span class="t-gray">@Reader A</span> dong y</div>
          <div cmtid="102"><a href="/user/2/">Reader B</a> -
          <span class="timeelap">Sun Jul 05 2026 10:01:00 GMT+0700</span></div>
        </div>
        """

        comments = eval_story_comments.parse_comments_html(
            html,
            story_id="1049330853",
            source="qidian",
        )

        self.assertEqual(len(comments), 2)
        self.assertEqual(comments[0]["comment_id"], "101")
        self.assertEqual(comments[0]["text"], "Truyen hay & cuon main thong minh")
        self.assertEqual(comments[0]["story_id"], "1049330853")
        self.assertEqual(comments[0]["source"], "qidian")
        self.assertEqual(comments[1]["comment_id"], "102")
        self.assertEqual(comments[1]["text"], "@Reader A dong y")

    def test_dedupe_comments_keeps_first_seen_id(self):
        comments = [
            {"source": "qidian", "story_id": "a", "comment_id": "1", "text": "first"},
            {"source": "qidian", "story_id": "a", "comment_id": "1", "text": "second"},
            {"source": "fanqie", "story_id": "b", "comment_id": "1", "text": "third"},
        ]

        deduped = eval_story_comments.dedupe_comments(comments)

        self.assertEqual([c["text"] for c in deduped], ["first", "third"])

    def test_story_urls_from_file_ignores_blank_lines_and_comments(self):
        lines = ["# stories", "", " http://example.test/a ", "http://example.test/b"]

        urls = eval_story_comments.story_urls_from_lines(lines)

        self.assertEqual(urls, ["http://example.test/a", "http://example.test/b"])


if __name__ == "__main__":
    unittest.main()
