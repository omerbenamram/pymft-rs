"""Checks the fields the Python classes expose against the Rust library's JSON for the same
entries, so a binding that reads the wrong field or converts it wrongly shows up here.

Not checked: `full_path` (the JSON has no path), and `type_code` and the integer `*_value` flag
fields other than the entry's, which are built from the same values as the names checked here."""

import datetime
import json
from pathlib import Path

import pytest

from mft import (
    PyMftAttributeNonResident,
    PyMftAttributeOther,
    PyMftAttributeX10,
    PyMftAttributeX20,
    PyMftAttributeX30,
    PyMftAttributeX40,
    PyMftAttributeX50,
    PyMftAttributeX60,
    PyMftAttributeX70,
    PyMftAttributeX80,
    PyMftAttributeX90,
    PyMftAttributeXC0,
    PyMftAttributeXD0,
    PyMftAttributeXE0,
    PyMftParser,
)

SAMPLES = Path(__file__).parent.parent / "samples"

EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)

ENTRY_FLAGS = {
    "ALLOCATED": 0x01,
    "INDEX_PRESENT": 0x02,
    "IS_EXTENSION": 0x04,
    "SPECIAL_INDEX_PRESENT": 0x08,
}

COLLATION_RULES = {
    "CollationBinary": 0x00,
    "CollationFilename": 0x01,
    "CollationUnicodeString": 0x02,
    "CollationNtofsUlong": 0x10,
    "CollationNtofsSid": 0x11,
    "CollationNtofsSecurityHash": 0x12,
    "CollationNtofsUlongs": 0x13,
}


def timestamp(value: str) -> datetime.datetime:
    """A JSON timestamp (100ns precision) as the bindings give it: truncated toward the epoch
    to whole microseconds."""
    whole, _, fraction = value.rstrip("Z").partition(".")
    seconds = datetime.datetime.fromisoformat(whole + "+00:00") - EPOCH
    ticks = (seconds.days * 86_400 + seconds.seconds) * 10_000_000 + int(fraction.ljust(7, "0"))
    micros = ticks // 10 if ticks >= 0 else -(-ticks // 10)
    return EPOCH + datetime.timedelta(microseconds=micros)


def assert_fields(obj, data, *fields):
    """Asserts each field of `obj` equals the JSON value of the same name."""
    for field in fields:
        assert getattr(obj, field) == data[field], field


def check_times(content, data):
    for field in ("created", "modified", "mft_modified", "accessed"):
        assert getattr(content, field) == timestamp(data[field]), field


def check_file_name(content, data):
    assert content.parent_entry_id == data["parent"]["entry"]
    assert content.parent_entry_sequence == data["parent"]["sequence"]
    assert_fields(
        content,
        data,
        "logical_size",
        "physical_size",
        "reparse_value",
        "namespace",
        "name",
        "flags",
    )
    check_times(content, data)


def check_acl(acl, data):
    if data is None:
        assert acl is None
        return
    assert acl.revision == data["revision"]
    for ace, expected in zip(acl.entries(), data["entries"], strict=True):
        assert_fields(
            ace,
            expected,
            "ace_type",
            "ace_type_name",
            "flags",
            "access_mask",
            "object_type",
            "inherited_object_type",
            "sid",
        )
        assert ace.data == bytes.fromhex(expected["data"])


VIEW_ENTRY_FIELDS = (
    "hash",
    "security_id",
    "sds_offset",
    "sds_length",
    "object_id",
    "birth_volume_id",
    "birth_object_id",
    "domain_id",
    "sid",
    "owner_id",
    "version",
    "quota_flags",
    "bytes_used",
    "warning_limit",
    "hard_limit",
    "tag",
    "tag_name",
)


def check_view_entry(entry, expected):
    assert entry.flags == expected["flags"]
    assert entry.value_type == expected["type"]
    for field in VIEW_ENTRY_FIELDS:
        assert getattr(entry, field) == expected.get(field), field
    reference = expected.get("mft_reference")
    assert entry.mft_reference_entry_id == (reference and reference["entry"])
    assert entry.mft_reference_entry_sequence == (reference and reference["sequence"])
    for field in ("change_time", "exceeded_time"):
        value = expected.get(field)
        assert getattr(entry, field) == (value and timestamp(value)), field
    for field in ("key", "data"):
        value = expected.get(field)
        assert getattr(entry, field) == (None if value is None else bytes.fromhex(value)), field


def check_content(content, data, header):
    if isinstance(content, PyMftAttributeX10):
        assert_fields(
            content,
            data,
            "max_version",
            "version",
            "class_id",
            "owner_id",
            "security_id",
            "quota",
            "usn",
            "file_flags",
        )
        check_times(content, data)
    elif isinstance(content, PyMftAttributeX20):
        for entry, expected in zip(content.entries(), data["entries"], strict=True):
            assert_fields(entry, expected, "attribute_type", "lowest_vcn", "name")
            assert entry.segment_entry_id == expected["segment_reference"]["entry"]
            assert entry.segment_entry_sequence == expected["segment_reference"]["sequence"]
    elif isinstance(content, PyMftAttributeX30):
        check_file_name(content, data)
    elif isinstance(content, PyMftAttributeX40):
        assert content.object_id == data["object_id"]
        # The bindings give a missing id as "" rather than None.
        for field in ("birth_volume_id", "birth_object_id", "domain_id"):
            assert getattr(content, field) == (data[field] or ""), field
    elif isinstance(content, PyMftAttributeX50):
        assert_fields(content, data, "revision", "control", "owner", "group")
        check_acl(content.sacl, data["sacl"])
        check_acl(content.dacl, data["dacl"])
    elif isinstance(content, PyMftAttributeX60):
        assert_fields(content, data, "name")
    elif isinstance(content, PyMftAttributeX70):
        assert_fields(content, data, "major_version", "minor_version", "flags")
    elif isinstance(content, PyMftAttributeX80):
        assert content.data == bytes.fromhex(data)
    elif isinstance(content, PyMftAttributeX90):
        assert_fields(
            content,
            data,
            "attribute_type",
            "index_entry_size",
            "index_entry_number_of_cluster_blocks",
            "relative_offset_to_index_node",
            "index_node_length",
            "index_node_allocation_length",
        )
        assert content.collation_rule == COLLATION_RULES[data["collation_rule"]]
        assert content.is_large_index == ("LARGE_INDEX" in data["index_root_flags"])
        expected_entries = data["index_entries"]["index_entries"]
        for entry, expected in zip(content.index_entries(), expected_entries, strict=True):
            assert entry.mft_reference_entry_id == expected["mft_reference"]["entry"]
            assert entry.mft_reference_entry_sequence == expected["mft_reference"]["sequence"]
            assert_fields(entry, expected, "index_record_length", "attr_fname_length", "flags")
            check_file_name(entry.fname_info, expected["fname_info"])
        expected_views = data["index_entries"].get("view_entries", [])
        for entry, expected in zip(content.view_entries(), expected_views, strict=True):
            check_view_entry(entry, expected)
    elif isinstance(content, PyMftAttributeXC0):
        assert_fields(content, data, "tag", "tag_name", "guid")
        reparse = data["data"]
        assert content.reparse_type == reparse["type"]
        for field in (
            "substitute_name",
            "print_name",
            "package_id",
            "app_user_model_id",
            "target_path",
            "target",
        ):
            assert getattr(content, field) == reparse.get(field), field
        assert content.is_relative == reparse.get("relative")
        if reparse["type"] == "Other":
            assert content.data == bytes.fromhex(reparse["data"])
        else:
            assert content.data is None
    elif isinstance(content, PyMftAttributeXD0):
        assert_fields(content, data, "packed_ea_size", "need_ea_count", "unpacked_ea_size")
    elif isinstance(content, PyMftAttributeXE0):
        for entry, expected in zip(content.entries(), data["entries"], strict=True):
            assert_fields(entry, expected, "flags", "name")
            value = expected["value"]
            assert entry.value_type == value["type"]
            for field in ("value", "major", "minor", "catalog"):
                assert getattr(entry, field) == value.get(field), field
            if value["type"] == "Other":
                assert entry.data == bytes.fromhex(value["data"])
            else:
                assert entry.data is None
    elif isinstance(content, PyMftAttributeNonResident):
        sizes = header["residential_header"]
        assert content.vcn_first == sizes["vnc_first"]
        assert content.vcn_last == sizes["vnc_last"]
        assert_fields(
            content,
            sizes,
            "allocated_length",
            "file_size",
            "valid_data_length",
            "unit_compression_size",
            "total_allocated",
        )
        for run, expected in zip(content.data_runs(), data["data_runs"], strict=True):
            assert_fields(run, expected, "lcn_offset", "lcn_length")
            assert run.is_sparse == (expected["run_type"] == "Sparse")
    elif isinstance(content, PyMftAttributeOther):
        assert content.data == bytes.fromhex(data["data"])
    else:
        raise AssertionError(f"unexpected content class {type(content).__name__}")


def check_entry(entry, expected):
    header = expected["header"]
    assert entry.entry_id == header["record_number"]
    assert entry.base_entry_id == header["base_reference"]["entry"]
    assert entry.base_entry_sequence == header["base_reference"]["sequence"]
    assert entry.lsn == header["metadata_transaction_journal"]
    assert_fields(
        entry,
        header,
        "sequence",
        "hard_link_count",
        "flags",
        "used_entry_size",
        "total_entry_size",
    )
    assert entry.valid_fixup == expected["valid_fixup"]
    flag_names = set(header["flags"].split(" | "))
    assert entry.flags_value == sum(bit for name, bit in ENTRY_FLAGS.items() if name in flag_names)
    assert entry.is_allocated == ("ALLOCATED" in flag_names)
    assert entry.is_dir == ("INDEX_PRESENT" in flag_names)
    first_data = next(
        (a["header"] for a in expected["attributes"] if a["header"]["type_code"] == "DATA"), None
    )
    if first_data is not None:
        sizes = first_data["residential_header"]
        assert entry.file_size == sizes.get("data_size", sizes.get("file_size"))

    # Python returns every attribute header and parses the content when it's read, while the
    # JSON lists an attribute whose content doesn't parse under `errors` instead.
    errors = 0
    parsed = []
    for attribute in entry.attributes():
        if isinstance(attribute, Exception):
            errors += 1
            continue
        try:
            parsed.append((attribute, attribute.attribute_content))
        except (RuntimeError, OSError):
            errors += 1
    assert errors == len(expected.get("errors", []))

    for (attribute, content), expected_attribute in zip(
        parsed, expected["attributes"], strict=True
    ):
        attr_header = expected_attribute["header"]
        assert attribute.type_name == attr_header["type_code"]
        assert attribute.data_size == attr_header["record_length"]
        assert_fields(attribute, attr_header, "name", "data_flags")
        assert attribute.is_resident == (attr_header["form_code"] == 0)
        if attribute.is_resident:
            raw_value = attribute.raw_value
            assert len(raw_value) == attr_header["residential_header"]["data_size"]
            if isinstance(content, (PyMftAttributeX80, PyMftAttributeOther)):
                assert raw_value == content.data
        else:
            assert attribute.raw_value is None
        check_content(content, expected_attribute["data"], attr_header)


@pytest.mark.parametrize("sample", ["MFT", "reparse_points", "entry_87354_extended_attributes"])
def test_python_fields_match_rust_json(sample: str):
    parser = PyMftParser(str(SAMPLES / sample))
    checked = 0
    for entry, expected in zip(parser.entries(), parser.entries_json(), strict=True):
        assert not isinstance(entry, Exception), entry
        check_entry(entry, json.loads(expected))
        checked += 1
    assert checked > 0
