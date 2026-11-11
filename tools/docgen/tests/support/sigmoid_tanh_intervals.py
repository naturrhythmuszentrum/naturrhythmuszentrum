# SPDX-License-Identifier: AGPL-3.0-only
"""Reproduce the directed route for the compact sigmoid and tanh golden rows.

Use mpmath 1.3.0, public revision b5c04506ef0cd4a1f1213f8389ee21c9c3551582.
Run with the repository root as the working directory. This reads no CAS
interval and checks only the separately retained mpmath route. It neither
imports a native evaluator nor changes the payload.
"""
import json
from pathlib import Path
from fractions import Fraction as F
import mpmath
from mpmath import iv

assert mpmath.__version__ == "1.3.0"
iv.dps = 150


def rational(t):
    """Read a directed binary endpoint as an exact rational."""
    sign, mantissa, exponent, _ = t
    return (-1 if sign else 1) * F(mantissa) * F(2) ** exponent


def floorlog(x):
    """Compute the exact floor of the binary logarithm of a positive rational."""
    k = x.numerator.bit_length() - x.denominator.bit_length()
    return k if x >= F(2) ** k else k - 1


def selected(row):
    """Select compact represented-operand rows, excluding the rational sigma box."""
    return row['id'].startswith(('sigma-', 'tanh-')) and row['id'] != 'sigma-ln-two-box'


result = {}
payload = json.loads(Path('data/golden/prepared-binary64.json').read_text())
for row in payload['entries']:
    if not selected(row) or row['expected'] != 'enclosure':
        continue
    for spelling in row['input'].get('box', [row['input'].get('point')]):
        key = row['operation'] + ':' + spelling
        if key in result:
            continue
        value = F(spelling)
        if not value:
            exact = '1/2' if row['operation'] == 'sigma' else '0'
            result[key] = [exact, exact]
            continue
        x = iv.mpf(value.numerator) / iv.mpf(value.denominator)
        assert rational(x._mpi_[0]) == value == rational(x._mpi_[1])
        if row['operation'] == 'sigma':
            y = 1 / (1 + iv.exp(-x))
            q = 128
        else:
            u = -2 * x
            if value <= F(1, 1024):
                term = u
                total = u
                for k in range(2, 41):
                    term = term * u / k
                    total = total + term
                lower = total + term * u / 41
                # Alternating terms decrease for |u| <= 1/512.
                # Odd partial sum is below expm1, even partial sum above.
                m = iv.mpf([lower.a, total.b])
            else:
                m = iv.exp(u) - 1
            y = -m / (2 + m)
            q = 128 + max(0, -floorlog(value))
        a, b = map(rational, y._mpi_)
        scale = 2 ** q
        lo = F((a * scale).__floor__(), scale)
        hi = F((b * scale).__ceil__(), scale)
        assert lo <= a <= b <= hi
        assert hi - lo <= F(1, 2 ** 120)
        if row['operation'] == 'tanh':
            assert hi - lo <= value * F(1, 2 ** 120)
        result[key] = [str(lo), str(hi)]
for row in payload['entries']:
    if not selected(row):
        continue
    for endpoint in row['evidence']:
        expected = result[row['operation'] + ':' + endpoint['input']]
        assert endpoint['routes'][1]['interval'] == expected, row['id']
print(json.dumps({'directed_endpoints': len(result), 'decimal_digits': iv.dps}))
