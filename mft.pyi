from typing import Any

class PyMftAttribute:
    attribute_content: Any
    data_flags: Any
    data_size: Any
    is_resident: Any
    name: Any
    type_code: Any
    type_name: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeOther:
    data: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeX10:
    accessed: Any
    class_id: Any
    created: Any
    file_flags: Any
    max_version: Any
    mft_modified: Any
    modified: Any
    owner_id: Any
    quota: Any
    security_id: Any
    usn: Any
    version: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeX20:
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...
    def entries(self, *args, **kwargs) -> Any: ...

class PyMftAttributeX30:
    accessed: Any
    created: Any
    flags: Any
    logical_size: Any
    mft_modified: Any
    modified: Any
    name: Any
    namespace: Any
    parent_entry_id: Any
    parent_entry_sequence: Any
    physical_size: Any
    reparse_value: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeX40:
    birth_object_id: Any
    birth_volume_id: Any
    domain_id: Any
    object_id: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeX80:
    data: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributeX90:
    attribute_type: Any
    collation_rule: Any
    index_entry_number_of_cluster_blocks: Any
    index_entry_size: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...

class PyMftAttributesIter:
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...
    def __iter__(self) -> Any: ...
    def __next__(self) -> Any: ...

class PyMftEntriesIterator:
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...
    def __iter__(self) -> Any: ...
    def __next__(self) -> Any: ...

class PyMftEntry:
    base_entry_id: Any
    base_entry_sequence: Any
    entry_id: Any
    file_size: Any
    flags: Any
    full_path: Any
    hard_link_count: Any
    sequence: Any
    total_entry_size: Any
    used_entry_size: Any
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...
    def attributes(self, *args, **kwargs) -> Any: ...

class PyMftParser:
    @classmethod
    def __init__(cls, *args, **kwargs) -> None: ...
    def entries(self, *args, **kwargs) -> Any: ...
    def entries_csv(self, *args, **kwargs) -> Any: ...
    def entries_json(self, *args, **kwargs) -> Any: ...
    def number_of_entries(self, *args, **kwargs) -> Any: ...
    def __iter__(self) -> Any: ...
    def __next__(self) -> Any: ...
