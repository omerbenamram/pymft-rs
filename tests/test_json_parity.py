"""Checks every field the Python classes expose against the Rust library's JSON for the same
entries, so a binding that reads the wrong field or converts it wrongly shows up here."""

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
    PyMftAttributeX60,
    PyMftAttributeX70,
    PyMftAttributeX80,
    PyMftAttributeX90,
    PyMftAttributeXC0,
    PyMftParser,
)

SAMPLES = Path(__file__).parent.parent / "samples"

EPOCH = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)

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


def check_times(content, data):
    for field in ("created", "modified", "mft_modified", "accessed"):
        assert getattr(content, field) == timestamp(data[field]), field


def check_file_name(content, data):
    assert content.parent_entry_id == data["parent"]["entry"]
    assert content.parent_entry_sequence == data["parent"]["sequence"]
    assert content.logical_size == data["logical_size"]
    assert content.physical_size == data["physical_size"]
    assert content.reparse_value == data["reparse_value"]
    assert content.namespace == data["namespace"]
    assert content.name == data["name"]
    assert content.flags == data["flags"]
    check_times(content, data)


def check_content(content, data, header):
    if isinstance(content, PyMftAttributeX10):
        for field in ("max_version", "version", "class_id", "owner_id", "security_id"):
            assert getattr(content, field) == data[field], field
        assert content.quota == data["quota"]
        assert content.usn == data["usn"]
        assert content.file_flags == data["file_flags"]
        check_times(content, data)
    elif isinstance(content, PyMftAttributeX20):
        entries = list(content.entries())
        assert len(entries) == len(data["entries"])
        for entry, expected in zip(entries, data["entries"]):
            assert entry.attribute_type == expected["attribute_type"]
            assert entry.lowest_vcn == expected["lowest_vcn"]
            assert entry.name == expected["name"]
            assert entry.segment_entry_id == expected["segment_reference"]["entry"]
            assert entry.segment_entry_sequence == expected["segment_reference"]["sequence"]
    elif isinstance(content, PyMftAttributeX30):
        check_file_name(content, data)
    elif isinstance(content, PyMftAttributeX40):
        for field in ("object_id", "birth_volume_id", "birth_object_id", "domain_id"):
            assert getattr(content, field) == (data[field] or ""), field
    elif isinstance(content, PyMftAttributeX60):
        assert content.name == data["name"]
    elif isinstance(content, PyMftAttributeX70):
        assert content.major_version == data["major_version"]
        assert content.minor_version == data["minor_version"]
        assert content.flags == data["flags"]
    elif isinstance(content, PyMftAttributeX80):
        assert content.data == bytes.fromhex(data)
    elif isinstance(content, PyMftAttributeX90):
        assert content.attribute_type == data["attribute_type"]
        assert content.collation_rule == COLLATION_RULES[data["collation_rule"]]
        for field in (
            "index_entry_size",
            "index_entry_number_of_cluster_blocks",
            "relative_offset_to_index_node",
            "index_node_length",
            "index_node_allocation_length",
        ):
            assert getattr(content, field) == data[field], field
        assert content.is_large_index == ("LARGE_INDEX" in data["index_root_flags"])
        entries = content.index_entries()
        expected_entries = data["index_entries"]["index_entries"]
        assert len(entries) == len(expected_entries)
        for entry, expected in zip(entries, expected_entries):
            assert entry.mft_reference_entry_id == expected["mft_reference"]["entry"]
            assert entry.mft_reference_entry_sequence == expected["mft_reference"]["sequence"]
            assert entry.index_record_length == expected["index_record_length"]
            assert entry.attr_fname_length == expected["attr_fname_length"]
            assert entry.flags == expected["flags"]
            check_file_name(entry.fname_info, expected["fname_info"])
    elif isinstance(content, PyMftAttributeXC0):
        assert content.tag == data["tag"]
        assert content.tag_name == data["tag_name"]
        assert content.guid == data["guid"]
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
    elif isinstance(content, PyMftAttributeNonResident):
        sizes = header["residential_header"]
        assert content.vcn_first == sizes["vnc_first"]
        assert content.vcn_last == sizes["vnc_last"]
        for field in (
            "allocated_length",
            "file_size",
            "valid_data_length",
            "unit_compression_size",
            "total_allocated",
        ):
            assert getattr(content, field) == sizes[field], field
        runs = content.data_runs()
        assert len(runs) == len(data["data_runs"])
        for run, expected in zip(runs, data["data_runs"]):
            assert run.lcn_offset == expected["lcn_offset"]
            assert run.lcn_length == expected["lcn_length"]
            assert run.is_sparse == (expected["run_type"] == "Sparse")
    elif isinstance(content, PyMftAttributeOther):
        assert content.data == bytes.fromhex(data["data"])
    else:
        raise AssertionError(f"unexpected content class {type(content).__name__}")


def check_entry(entry, expected):
    header = expected["header"]
    assert entry.entry_id == header["record_number"]
    assert entry.sequence == header["sequence"]
    assert entry.base_entry_id == header["base_reference"]["entry"]
    assert entry.base_entry_sequence == header["base_reference"]["sequence"]
    assert entry.hard_link_count == header["hard_link_count"]
    assert entry.flags == header["flags"]
    assert entry.lsn == header["metadata_transaction_journal"]
    assert entry.used_entry_size == header["used_entry_size"]
    assert entry.total_entry_size == header["total_entry_size"]
    assert entry.valid_fixup == expected["valid_fixup"]

    attributes = list(entry.attributes())
    errors = [a for a in attributes if isinstance(a, Exception)]
    attributes = [a for a in attributes if not isinstance(a, Exception)]
    assert len(errors) == len(expected.get("errors", []))
    assert len(attributes) == len(expected["attributes"])

    for attribute, expected_attribute in zip(attributes, expected["attributes"]):
        attr_header = expected_attribute["header"]
        assert attribute.type_name == attr_header["type_code"]
        assert attribute.name == attr_header["name"]
        assert attribute.data_flags == attr_header["data_flags"]
        assert attribute.data_size == attr_header["record_length"]
        assert attribute.is_resident == (attr_header["form_code"] == 0)
        if attribute.is_resident:
            raw_value = attribute.raw_value
            assert len(raw_value) == attr_header["residential_header"]["data_size"]
        else:
            assert attribute.raw_value is None
        check_content(attribute.attribute_content, expected_attribute["data"], attr_header)


@pytest.mark.parametrize("sample", ["MFT", "reparse_points"])
def test_python_fields_match_rust_json(sample: str):
    parser = PyMftParser(str(SAMPLES / sample))
    checked = 0
    for entry, expected in zip(parser.entries(), parser.entries_json(), strict=True):
        assert not isinstance(entry, Exception), entry
        check_entry(entry, json.loads(expected))
        checked += 1
    assert checked > 0
