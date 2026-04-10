# Changelog Edits

When adding a changelog entry, use the declared instructions.
However, when cutting a release, the rules are different.

## Rules For Cutting A Release

When cutting a release, the entries for the new version should be aggregated based on several factors:
1. First, tightly related entries are merged. For example, if there are multiple entries related to new rule additions in Judge, they can be combined into a single entry that summarizes the overall improvement to Judge's rule system.
2. Then, entries that are of no interest to any end user (for example, internal refactors that don't impact functionality or user experience) are removed.
3. Entries are verified to be in the correct category section (Added/Changed/Fixed/etc.) and are moved if necessary.
4. Within a category, entries are grouped by area (for example, by sub-projects like Judge and Debi) and ordered within the group based on their relative importance and impact.

The goal of these edits is to create a concise and user-friendly changelog that highlights the most important changes and improvements in each release, while still providing enough detail for users who want to understand the specifics.
