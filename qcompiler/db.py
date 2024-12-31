
from __future__ import annotations

from dataclasses import dataclass
from typing import TypeVar, Dict

K = TypeVar('K')
V = TypeVar('V')

@dataclass
class Claim:
    key: K
    db: Db

    def insert(self, value) -> V:
        pass


class Db:
    def __init__(self):
        self._data: Dict[K, V] = dict()

    def claim(self, k) -> Claim | V:
        if k in self._data:
            return self._data[k]
        return Claim(k, self)

    def peek(self, k) -> V | None:
        pass


