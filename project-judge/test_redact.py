#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
# ]
# ///

"""Tests for redact.py"""

import hashlib
import unittest

from redact import (
    SAFE_PATH_SEGMENTS,
    generalize_path_segments,
    redact_entry,
    redact_git_hashes,
    redact_quoted_tool_args,
    redact_timestamps,
    audit_entry,
)


class TestDescriptionRemoval(unittest.TestCase):
    """Feature 1: Remove tool_input.description during redaction."""

    def test_description_removed_from_tool_input(self):
        entry = {
            "cwd": "/home/user/workspace/proj",
            "tool_input": {
                "command": "ls",
                "description": "List files in current directory",
            },
        }
        result = redact_entry(entry)
        self.assertNotIn("description", result["tool_input"])

    def test_entry_without_description_is_unchanged(self):
        entry = {
            "cwd": "/home/user/workspace/proj",
            "tool_input": {
                "command": "ls",
            },
        }
        result = redact_entry(entry)
        self.assertEqual(result["tool_input"], {"command": "ls"})

    def test_top_level_description_not_removed(self):
        """description removal only applies inside tool_input, not at top level."""
        entry = {
            "cwd": "/home/user/workspace/proj",
            "description": "some top-level field",
            "tool_input": {
                "command": "ls",
            },
        }
        result = redact_entry(entry)
        self.assertIn("description", result)


class TestGitHashRedaction(unittest.TestCase):
    """Feature 2a: Git commit hash redaction."""

    def test_short_hash_redacted(self):
        text = "git show d209459c22a --stat"
        result = redact_git_hashes(text)
        self.assertNotIn("d209459c22a", result)
        # Deterministic: same input => same output
        self.assertEqual(result, redact_git_hashes(text))

    def test_full_hash_redacted(self):
        full_hash = "a" * 40
        text = f"commit {full_hash}"
        result = redact_git_hashes(text)
        self.assertNotIn(full_hash, result)
        # Replacement should be uppercase hex of same length
        import re
        found = re.search(r'[0-9A-F]{40}', result)
        self.assertIsNotNone(found)
        self.assertEqual(len(found.group(0)), 40)

    def test_short_hex_words_not_matched(self):
        """Words like 'cafe' (4 chars) must not be matched (min 7 chars)."""
        text = "cafe babe dead beef"
        result = redact_git_hashes(text)
        self.assertEqual(text, result)

    def test_hash_command_not_affected(self):
        """The literal command 'hash' must not be touched by git hash redaction."""
        text = "hash"
        result = redact_git_hashes(text)
        self.assertEqual("hash", result)

    def test_multiple_hashes_in_same_string(self):
        text = "git show d209459c22a --stat && git show 92ed21fda29 --stat"
        result = redact_git_hashes(text)
        self.assertNotIn("d209459c22a", result)
        self.assertNotIn("92ed21fda29", result)

    def test_deterministic_replacement(self):
        text = "d209459c22a"
        result = redact_git_hashes(text)
        self.assertNotEqual(text, result)
        # Must be uppercase
        self.assertEqual(result, result.upper())
        # Must be deterministic: same input always produces same output
        self.assertEqual(result, redact_git_hashes(text))

    def test_idempotent_replacement(self):
        """Applying redaction twice should give the same result.

        Fake hashes are uppercase so they don't re-match the lowercase pattern.
        """
        text = "git show d209459c22a --stat"
        once = redact_git_hashes(text)
        twice = redact_git_hashes(once)
        self.assertEqual(once, twice)

    def test_project_hash_not_redacted(self):
        """project-XXXXX hashes (5 uppercase hex chars) should not be redacted."""
        # These are 5 chars, well below the 7-char minimum, so they won't match.
        text = "project-A1B2C"
        result = redact_git_hashes(text)
        self.assertEqual(text, result)


class TestQuotedToolArgRedaction(unittest.TestCase):
    """Feature 2b: Quoted string redaction after gaac/submit-pr."""

    def test_gaac_quoted_arg_redacted(self):
        text = 'gaac "Add configurable paste_override for p key"'
        result = redact_quoted_tool_args(text)
        self.assertEqual('gaac "<redacted>"', result)

    def test_submit_pr_all_args_redacted(self):
        text = 'submit-pr "PR title" "PR body goes here"'
        result = redact_quoted_tool_args(text)
        self.assertEqual('submit-pr "<redacted>"', result)

    def test_submit_pr_with_multiline_body(self):
        text = 'submit-pr "Fix footer" "## Summary\n\n- ...'
        result = redact_quoted_tool_args(text)
        self.assertEqual('submit-pr "<redacted>"', result)

    def test_non_tool_quoted_strings_not_affected(self):
        text = 'python -c "for i in range(5): print(i)"'
        result = redact_quoted_tool_args(text)
        self.assertEqual(text, result)

    def test_gaac_without_quotes_not_affected(self):
        text = "gaac something"
        result = redact_quoted_tool_args(text)
        self.assertEqual(text, result)


class TestTimestampRedaction(unittest.TestCase):
    """Feature 2c: Timestamp redaction in filenames."""

    def test_timestamp_redacted(self):
        text = "/tmp/devora_crash_20260312_113150.log"
        result = redact_timestamps(text)
        self.assertEqual("/tmp/devora_crash_00000000_000000.log", result)

    def test_timestamp_with_dash_separator(self):
        text = "file_20260312-113150.log"
        result = redact_timestamps(text)
        self.assertEqual("file_00000000_000000.log", result)

    def test_no_timestamp_unchanged(self):
        text = "just a normal string"
        result = redact_timestamps(text)
        self.assertEqual(text, result)


class TestPathSegmentGeneralization(unittest.TestCase):
    """Feature 3: Path segment generalization with hashing."""

    def _expected_hash(self, segment):
        return hashlib.sha256(segment.encode()).hexdigest()[:5].upper()

    def test_safe_segments_pass_through(self):
        for segment in SAFE_PATH_SEGMENTS:
            text = f"/home/user/{segment}/foo"
            result = generalize_path_segments(text)
            self.assertIn(f"/{segment}/", result, f"Safe segment {segment} should pass through")

    def test_project_segment_hashed(self):
        text = "/home/user/my-secret-project/src/main.py"
        result = generalize_path_segments(text)
        expected_hash = self._expected_hash("my-secret-project")
        self.assertIn(f"project-{expected_hash}", result)
        self.assertNotIn("my-secret-project", result)
        # 'src' is safe, should remain
        self.assertIn("/src/", result)

    def test_filename_hashed_with_extension_kept(self):
        text = "/home/user/workspace/main.py"
        result = generalize_path_segments(text)
        expected_hash = self._expected_hash("main")
        self.assertIn(f"file-{expected_hash}.py", result)
        self.assertNotIn("main.py", result)

    def test_filename_without_extension_treated_as_segment(self):
        """A last segment without a dot extension is treated as a directory segment."""
        text = "/home/user/workspace/mydir"
        result = generalize_path_segments(text)
        expected_hash = self._expected_hash("mydir")
        self.assertIn(f"project-{expected_hash}", result)

    def test_cwd_and_command_use_same_hash(self):
        """Paths in cwd and command must hash the same segment identically."""
        entry = {
            "cwd": "/home/user/workspace/my-project",
            "tool_input": {
                "command": "cd /home/user/workspace/my-project && ls",
            },
        }
        result = redact_entry(entry)
        # Extract the hashed project name from cwd
        cwd_project = result["cwd"].split("/")[3]  # /home/user/<hashed>/...
        # The same hash should appear in the command
        self.assertIn(cwd_project, result["tool_input"]["command"])

    def test_tmp_paths_hashed(self):
        text = "/tmp/render_stdout.log"
        result = generalize_path_segments(text)
        expected_hash = self._expected_hash("render_stdout")
        self.assertIn(f"/tmp/file-{expected_hash}.log", result)
        self.assertNotIn("render_stdout", result)

    def test_tmp_path_without_extension(self):
        text = "/tmp/somefile"
        result = generalize_path_segments(text)
        expected_hash = self._expected_hash("somefile")
        self.assertIn(f"/tmp/file-{expected_hash}", result)

    def test_home_user_prefix_preserved(self):
        text = "/home/user/workspace/proj/file.txt"
        result = generalize_path_segments(text)
        self.assertTrue(result.startswith("/home/user/"))

    def test_deep_nested_path(self):
        text = "/home/user/dev/repos/my-repo/subdir/module.py"
        result = generalize_path_segments(text)
        self.assertNotIn("my-repo", result)
        self.assertNotIn("subdir", result)
        self.assertNotIn("module.py", result)
        # But safe segments would be preserved if present
        self.assertIn("/home/user/", result)


class TestRedactEntryIntegration(unittest.TestCase):
    """Integration tests for the full redact_entry pipeline."""

    def test_full_pipeline_order(self):
        """All features work together in correct order."""
        entry = {
            "cwd": "/home/user/workspace/my-project",
            "tool_input": {
                "command": 'git show d209459c22a --stat',
                "description": "Show commit info",
            },
        }
        result = redact_entry(entry)
        # Description removed
        self.assertNotIn("description", result["tool_input"])
        # Git hash redacted
        self.assertNotIn("d209459c22a", result["tool_input"]["command"])
        # Path generalized
        self.assertNotIn("my-project", result["cwd"])

    def test_hash_command_survives_full_pipeline(self):
        """The literal 'hash' command must survive all redaction steps."""
        entry = {
            "cwd": "/home/user/workspace/myproj",
            "tool_input": {
                "command": "hash",
            },
        }
        result = redact_entry(entry)
        self.assertEqual("hash", result["tool_input"]["command"])

    def test_gaac_with_path_and_hash(self):
        """Entry with gaac, paths, and potential hashes."""
        entry = {
            "cwd": "/home/user/workspace/my-project",
            "tool_input": {
                "command": 'gaac "Fix bug in parser"',
                "description": "Commit all changes",
            },
        }
        result = redact_entry(entry)
        self.assertNotIn("description", result["tool_input"])
        self.assertIn("<redacted>", result["tool_input"]["command"])
        self.assertNotIn("my-project", result["cwd"])


class TestAudit(unittest.TestCase):
    """Feature 4: Audit mode checks."""

    def test_clean_entry_has_no_findings(self):
        """A properly redacted entry should produce no audit findings."""
        entry = {
            "cwd": "/home/user/src/project-A1B2C",
            "tool_input": {
                "command": "ls /home/user/src/project-A1B2C",
            },
        }
        findings = audit_entry(entry, index=0)
        self.assertEqual(findings, [])

    def test_description_found_in_audit(self):
        entry = {
            "cwd": "/home/user/project-A1B2C",
            "tool_input": {
                "command": "ls",
                "description": "something",
            },
        }
        findings = audit_entry(entry, index=0)
        descriptions = [f for f in findings if "description" in f.lower()]
        self.assertTrue(len(descriptions) > 0)

    def test_suspicious_path_segment_found(self):
        entry = {
            "cwd": "/home/user/my-real-project-name/src",
            "tool_input": {
                "command": "ls",
            },
        }
        findings = audit_entry(entry, index=0)
        self.assertTrue(len(findings) > 0)

    def test_timestamp_found_in_audit(self):
        entry = {
            "cwd": "/home/user/project-A1B2C",
            "tool_input": {
                "command": "stat /tmp/file_20260312_113150.log",
            },
        }
        findings = audit_entry(entry, index=0)
        timestamp_findings = [f for f in findings if "timestamp" in f.lower()]
        self.assertTrue(len(timestamp_findings) > 0)

    def test_long_quoted_string_found_in_audit(self):
        entry = {
            "cwd": "/home/user/project-A1B2C",
            "tool_input": {
                "command": 'gaac "This is a long commit message that should be caught"',
            },
        }
        findings = audit_entry(entry, index=0)
        quoted_findings = [f for f in findings if "quoted" in f.lower()]
        self.assertTrue(len(quoted_findings) > 0)

    def test_redacted_quoted_string_not_flagged(self):
        entry = {
            "cwd": "/home/user/project-A1B2C",
            "tool_input": {
                "command": 'gaac "<redacted>"',
            },
        }
        findings = audit_entry(entry, index=0)
        quoted_findings = [f for f in findings if "quoted" in f.lower()]
        self.assertEqual(len(quoted_findings), 0)


if __name__ == "__main__":
    unittest.main()
