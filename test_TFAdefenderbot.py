"""
Copyright (C) 2020 Kunal Mehta

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
"""
import os; os.environ['PYWIKIBOT_NO_USER_CONFIG'] = '1'  # noqa
import pytest

import pywikibot
import TFAdefenderbot


@pytest.mark.parametrize('page,expected', (
    ('Albert Einstein', {
        'edit': {
            'expiry': 'infinity',
            'level': 'autoconfirmed',
            'type': 'edit'
        },
        'move': {
            'expiry': 'infinity',
            'level': 'sysop',
            'type': 'move'
        }
    }),
    ('User:Legoktm/test', {})
))
def test_prot_status(page, expected):
    # A relatively stable page
    pg = pywikibot.Page(TFAdefenderbot.enwp, page)
    status = TFAdefenderbot.prot_status(pg)
    assert status == expected
