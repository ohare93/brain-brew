import copy
import json
import pathlib
import tempfile
import unittest

from scripts import ug_fixture


class UltimateGeographyFixtureBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp_dir.name)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_tree_metadata_detects_source_drift(self):
        source = self.root / "source"
        source.mkdir()
        (source / "deck.yaml").write_text("name: original\n", encoding="utf-8")
        recorded = ug_fixture.tree_metadata(source)

        (source / "deck.yaml").write_text("name: drifted!\n", encoding="utf-8")

        with self.assertRaisesRegex(ug_fixture.FixtureError, "source tree digest drift"):
            ug_fixture.require_tree_metadata(source, recorded, "source tree")

    def test_attribution_record_detects_license_or_source_drift(self):
        source = self.root / "source"
        source.mkdir()
        for name in ug_fixture.ATTRIBUTION_FILES:
            (source / name).write_text(f"pinned {name}\n", encoding="utf-8")
        recorded = ug_fixture.selected_tree_metadata(
            source,
            ug_fixture.ATTRIBUTION_FILES,
            label="UG third-party attribution",
        )
        recorded["paths"] = list(ug_fixture.ATTRIBUTION_FILES)
        sources = source / "sources.csv"
        original = sources.read_text(encoding="utf-8")
        sources.write_text(f"X{original[1:]}", encoding="utf-8")

        with self.assertRaisesRegex(
            ug_fixture.FixtureError, "third-party attribution sha256 drift"
        ):
            ug_fixture._validate_attribution_record(source, recorded)

    def test_hardcore_supplement_record_detects_vendored_drift(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        committed = (
            repo_root
            / "fixtures"
            / "ultimate-geography-attribution"
            / "hardcore-geography"
        )
        copied = self.root / "repo" / "fixtures"
        for component in ug_fixture.HARDCORE_ATTRIBUTION_ROOT_PARTS:
            copied /= component
        copied.mkdir(parents=True)
        for name in ug_fixture.HARDCORE_ATTRIBUTION_FILES:
            (copied / name).write_bytes((committed / name).read_bytes())
        sources = copied / "sources.csv"
        sources.write_bytes(sources.read_bytes() + b"drift")

        with self.assertRaisesRegex(
            ug_fixture.FixtureError,
            "Hardcore Geography attribution supplement .* drift",
        ):
            ug_fixture._validate_hardcore_attribution_supplement(
                self.root / "repo",
                ug_fixture._reviewed_hardcore_attribution_supplement(),
            )

    def test_committed_fixture_state_matches_lock(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]

        lock = ug_fixture.validate_fixture_state(repo_root)

        self.assertEqual(lock["schema_version"], 3)
        self.assertEqual(lock["source"]["file_count"], 736)
        self.assertEqual(lock["source"]["third_party_attribution"]["file_count"], 2)
        supplement = lock["attribution"]["supplements"]["hardcore_geography"]
        self.assertEqual(supplement["file_count"], 2)
        self.assertEqual(supplement["byte_count"], 6_126)
        self.assertEqual(
            supplement["provenance"]["revision"],
            ug_fixture.PINNED_HARDCORE_REVISION,
        )
        self.assertEqual(lock["attribution"]["coverage"]["media_file_count"], 607)
        self.assertEqual(lock["attribution"]["coverage"]["image_file_count"], 602)
        self.assertEqual(lock["expected"]["file_count"], 100)

    def test_committed_media_attribution_inventory_is_exact(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        fixture_root = repo_root / "fixtures" / ug_fixture.SOURCE_ROOT_NAME
        supplement_root = repo_root / "fixtures"
        for component in ug_fixture.HARDCORE_ATTRIBUTION_ROOT_PARTS:
            supplement_root /= component

        coverage = ug_fixture.attribution_coverage_metadata(
            fixture_root / "media",
            fixture_root / "sources.csv",
            supplement_root / "sources.csv",
        )

        self.assertEqual(coverage["media_file_count"], 607)
        self.assertEqual(coverage["image_file_count"], 602)
        self.assertEqual(coverage["ultimate_geography"]["sources_csv_file_count"], 546)
        self.assertEqual(coverage["hardcore_geography"]["sources_csv_file_count"], 56)
        self.assertEqual(coverage["hardcore_geography"]["flag_file_count"], 39)
        self.assertEqual(coverage["hardcore_geography"]["map_file_count"], 17)
        self.assertEqual(coverage["unattributed_file_count"], 0)
        self.assertEqual(coverage["ambiguous_file_count"], 0)

    def test_attribution_inventory_rejects_ambiguous_normalized_filename(self):
        media = self.root / "media"
        media.mkdir()
        (media / "same.svg").write_bytes(b"image")
        ug_sources = self.root / "ug.csv"
        hardcore_sources = self.root / "hardcore.csv"
        self._write_sources_csv(ug_sources, ["same.svg"])
        self._write_sources_csv(hardcore_sources, ["same.svg"])

        with self.assertRaisesRegex(ug_fixture.FixtureError, "ambiguous attribution"):
            ug_fixture.attribution_coverage_metadata(
                media,
                ug_sources,
                hardcore_sources,
                ug_notice_files=(),
                enforce_release_counts=False,
            )

    def test_attribution_inventory_rejects_unattributed_media(self):
        media = self.root / "media"
        media.mkdir()
        (media / "missing.svg").write_bytes(b"image")
        ug_sources = self.root / "ug.csv"
        hardcore_sources = self.root / "hardcore.csv"
        self._write_sources_csv(ug_sources, [])
        self._write_sources_csv(hardcore_sources, [])

        with self.assertRaisesRegex(ug_fixture.FixtureError, "unattributed media"):
            ug_fixture.attribution_coverage_metadata(
                media,
                ug_sources,
                hardcore_sources,
                ug_notice_files=(),
                enforce_release_counts=False,
            )

    def test_attribution_inventory_rejects_unknown_source_row(self):
        media = self.root / "media"
        media.mkdir()
        (media / "known.svg").write_bytes(b"image")
        ug_sources = self.root / "ug.csv"
        hardcore_sources = self.root / "hardcore.csv"
        self._write_sources_csv(ug_sources, ["known.svg", "unknown.svg"])
        self._write_sources_csv(hardcore_sources, [])

        with self.assertRaisesRegex(
            ug_fixture.FixtureError, "attribution entries have no vendored media"
        ):
            ug_fixture.attribution_coverage_metadata(
                media,
                ug_sources,
                hardcore_sources,
                ug_notice_files=(),
                enforce_release_counts=False,
            )

    def test_attribution_filename_normalization_fails_on_non_nfc_drift(self):
        with self.assertRaisesRegex(ug_fixture.FixtureError, "Unicode NFC"):
            ug_fixture.normalize_attribution_filename(
                "ug-flag-cafe\u0301.svg", "test filename"
            )

    def test_modified_binary_that_prints_pinned_version_is_rejected(self):
        binary = self.root / "modified-brainbrew"
        binary.write_text(
            "#!/bin/sh\nprintf '%s\\n' 'brainbrew 1.0.0-alpha.3'\n",
            encoding="utf-8",
        )
        binary.chmod(0o755)

        with self.assertRaisesRegex(
            ug_fixture.FixtureError, "executable digest mismatch"
        ):
            ug_fixture._verify_brainbrew_binary(
                binary,
                ug_fixture.PINNED_BRAINBREW_REVISION,
                self.root,
            )

    def test_lock_only_generator_self_blessing_is_rejected(self):
        generator = copy.deepcopy(ug_fixture._reviewed_generator())
        generator["executable"]["sha256"] = "0" * 64

        with self.assertRaisesRegex(ug_fixture.FixtureError, "generator identity drifted"):
            ug_fixture._validate_generator_record(generator)

    def test_descendant_source_cannot_masquerade_as_pinned_generator(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]

        with self.assertRaisesRegex(
            ug_fixture.FixtureError, "generator source identity mismatch"
        ):
            ug_fixture._validate_brainbrew_source(repo_root)

    def test_source_contract_rejects_intentional_exclusion_provenance_drift(self):
        source = {
            "root": ug_fixture.SOURCE_ROOT_NAME,
            "whitelist": list(ug_fixture.SOURCE_WHITELIST),
            "intentional_exclusions": ug_fixture._intentional_exclusions(),
        }
        source["intentional_exclusions"][0]["reason"] = "silently changed"

        with self.assertRaisesRegex(
            ug_fixture.FixtureError, "intentional-exclusion provenance drift"
        ):
            ug_fixture._validate_source_record_contract(source)

    def test_expected_validation_rejects_count_preserving_value_substitution(self):
        expected = self._expected_tree(["en-standard", "de-standard"])
        mapping = {
            "brainbrew.yaml": ["de-standard", "en-standard"],
            "brainbrew-hardcore.yaml": [],
        }
        recorded = ug_fixture.json_tree_metadata(expected)
        deck_path = expected / "en-standard" / "deck.json"
        value = json.loads(deck_path.read_text(encoding="utf-8"))
        value["name"] = "substituted"
        deck_path.write_text(json.dumps(value), encoding="utf-8")

        with self.assertRaisesRegex(ug_fixture.FixtureError, "expected JSON semantic digest drift"):
            ug_fixture.validate_expected_tree(expected, mapping, recorded, enforce_release_counts=False)

    def test_expected_validation_rejects_missing_target(self):
        expected = self._expected_tree(["en-standard", "de-standard"])
        mapping = {
            "brainbrew.yaml": ["de-standard", "en-standard"],
            "brainbrew-hardcore.yaml": [],
        }
        recorded = ug_fixture.json_tree_metadata(expected)
        deck_path = expected / "de-standard" / "deck.json"
        deck_path.unlink()
        deck_path.parent.rmdir()

        with self.assertRaisesRegex(ug_fixture.FixtureError, "expected target set drift"):
            ug_fixture.validate_expected_tree(expected, mapping, recorded, enforce_release_counts=False)

    def test_expected_validation_rejects_extra_target(self):
        expected = self._expected_tree(["en-standard", "de-standard"])
        mapping = {
            "brainbrew.yaml": ["de-standard", "en-standard"],
            "brainbrew-hardcore.yaml": [],
        }
        recorded = ug_fixture.json_tree_metadata(expected)
        extra = expected / "unexpected-target"
        extra.mkdir()
        (extra / "deck.json").write_text('{"name":"extra"}\n', encoding="utf-8")

        with self.assertRaisesRegex(ug_fixture.FixtureError, "expected target set drift"):
            ug_fixture.validate_expected_tree(expected, mapping, recorded, enforce_release_counts=False)

    def test_expected_digest_ignores_json_key_order_and_whitespace(self):
        expected = self._expected_tree(["en-standard"])
        before = ug_fixture.json_tree_metadata(expected)
        deck_path = expected / "en-standard" / "deck.json"
        deck_path.write_text('{\n  "notes": [],\n  "name": "en-standard"\n}\n', encoding="utf-8")

        self.assertEqual(ug_fixture.json_tree_metadata(expected), before)

    def test_source_sync_preserves_expected_outputs_and_acceptance_record(self):
        checkout = self.root / "ug"
        checkout.mkdir()
        for name in ug_fixture.SOURCE_WHITELIST:
            path = checkout / name
            if "." in pathlib.PurePath(name).name:
                path.write_text(f"pinned bytes for {name}\n", encoding="utf-8")
            else:
                path.mkdir()
                (path / "owned.txt").write_text(f"pinned {name}\n", encoding="utf-8")
        (checkout / "brainbrew.yaml").write_text(
            self._manifest_source("main", 74), encoding="utf-8"
        )
        (checkout / "brainbrew-hardcore.yaml").write_text(
            self._manifest_source("companion", 26), encoding="utf-8"
        )

        fixtures = self.root / "repo/fixtures"
        fixtures.mkdir(parents=True)
        expected = fixtures / "ultimate-geography-expected/crowdanki/en-standard"
        expected.mkdir(parents=True)
        sentinel_bytes = b'{"sentinel":"must not be blessed"}\n'
        (expected / "deck.json").write_bytes(sentinel_bytes)
        supplement = fixtures
        for component in ug_fixture.HARDCORE_ATTRIBUTION_ROOT_PARTS:
            supplement /= component
        supplement.mkdir(parents=True)
        supplement_sentinel = b"separately pinned Hardcore attribution\n"
        (supplement / "README.md").write_bytes(supplement_sentinel)

        lock = ug_fixture._new_lock()
        lock["expected"]["sentinel"] = "preserve-me"
        attribution_record = copy.deepcopy(lock["attribution"])
        (fixtures / ug_fixture.LOCK_NAME).write_text(
            json.dumps(lock), encoding="utf-8"
        )

        refreshed = ug_fixture.sync_source(
            self.root / "repo", checkout, ug_fixture.PINNED_UG_REVISION
        )

        self.assertEqual(refreshed["expected"], lock["expected"])
        self.assertEqual(refreshed["attribution"], attribution_record)
        self.assertEqual((expected / "deck.json").read_bytes(), sentinel_bytes)
        self.assertEqual((supplement / "README.md").read_bytes(), supplement_sentinel)
        self.assertEqual(
            (fixtures / "ultimate-geography/deck.yaml").read_bytes(),
            (checkout / "deck.yaml").read_bytes(),
        )

    def test_manifest_target_parser_reads_only_top_level_targets(self):
        manifest = self.root / "brainbrew.yaml"
        manifest.write_text(
            "package:\n"
            "  id: example\n"
            "targets:\n"
            "  en-standard:\n"
            "    overlays: []\n"
            "  de-standard:\n"
            "    overlays: []\n"
            "languages:\n"
            "  de:\n"
            "    targets:\n"
            "      standard: de-standard\n",
            encoding="utf-8",
        )

        self.assertEqual(
            ug_fixture.manifest_targets(manifest),
            ["de-standard", "en-standard"],
        )

    @staticmethod
    def _write_sources_csv(path, filenames):
        rows = ["File,Source,License,Modifications"]
        rows.extend(
            f"{filename},https://example.invalid/{filename},PD," for filename in filenames
        )
        path.write_text("\n".join(rows) + "\n", encoding="utf-8")

    @staticmethod
    def _manifest_source(prefix, count):
        targets = "".join(
            f"  {prefix}-{index:03d}:\n    overlays: []\n" for index in range(count)
        )
        return f"base: deck.yaml\noverlays: {{}}\ntargets:\n{targets}"

    def _expected_tree(self, targets):
        expected = self.root / "expected"
        expected.mkdir()
        for target in targets:
            target_dir = expected / target
            target_dir.mkdir()
            (target_dir / "deck.json").write_text(
                json.dumps({"name": target, "notes": []}) + "\n",
                encoding="utf-8",
            )
        return expected


if __name__ == "__main__":
    unittest.main()
