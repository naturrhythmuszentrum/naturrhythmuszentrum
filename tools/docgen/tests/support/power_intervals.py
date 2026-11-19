# SPDX-License-Identifier: AGPL-3.0-only
"""Manually reproduce directed dyadic-power golden intervals with mpmath 1.3.0.

Revision b5c04506ef0cd4a1f1213f8389ee21c9c3551582, pure Python backend.
Only authored inputs, exponents and the directed route are read. No CAS result,
certificate, native evaluator or CAS numerical helper participates.
"""
import json
from fractions import Fraction as F
from pathlib import Path
import mpmath
from mpmath import iv


def rational(t):
    """Decode an interval-library dyadic endpoint exactly."""
    sign, mantissa, exponent, _ = t
    return (-1 if sign else 1) * F(mantissa) * F(2) ** exponent


def evaluate(x, q, digits):
    """Take positive roots, then integer powers, with directed operations."""
    iv.dps = digits
    value = iv.mpf(x.numerator) / iv.mpf(x.denominator)
    assert rational(value._mpi_[0]) <= x <= rational(value._mpi_[1])
    denominator = q.denominator
    assert denominator & (denominator - 1) == 0 and 0 < q <= 1
    for _ in range(denominator.bit_length() - 1):
        value = iv.sqrt(value)
    product = iv.mpf(1)
    for bit in bin(q.numerator)[2:]:
        product = product * product
        if bit == '1':
            product = product * value
    return tuple(map(rational, product._mpi_))


def derive(text, exponent):
    """Refine independently, then coarsen outward to a finite dyadic grid."""
    assert mpmath.__version__ == '1.3.0'
    x, q = F(text), F(exponent)
    coarse = evaluate(x, q, 80)
    lo, hi = evaluate(x, q, 150)
    assert coarse[0] <= lo <= hi <= coarse[1], 'directed refinement'
    k = x.numerator.bit_length() - x.denominator.bit_length()
    k -= x < F(2) ** k
    scale = 1 << (128 + max(0, -k))
    lower, upper = F((lo * scale).__floor__(), scale), F((hi * scale).__ceil__(), scale)
    assert lower <= lo <= hi <= upper, 'outward coarsening'
    assert upper - lower <= F(1, 2**120), 'endpoint width'
    return [str(lower), str(upper)]


def main():
    payload = json.loads(Path('data/golden/dyadic-power.json').read_text())
    seen = {}
    for row in payload['entries']:
        for evidence in row['evidence']:
            pair = derive(evidence['input'], row['exponent'])
            assert evidence['routes'][1]['interval'] == pair, row['id']
            seen[(evidence['input'], row['exponent'])] = None
    print(json.dumps({'directed_endpoints': len(seen), 'decimal_digits': 150}))


if __name__ == '__main__':
    main()
