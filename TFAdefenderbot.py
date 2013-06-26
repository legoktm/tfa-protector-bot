#!/usr/bin/env python
from __future__ import unicode_literals
"""
Copyright (C) 2013 Legoktm

Released as CC-Zero.
"""
import datetime
import pywikibot
from pywikibot.data import api
from pywikibot import config

config.put_throttle = 0
config.maxlag = 999999999  # Don't worry about it

enwp = pywikibot.Site('en', 'wikipedia')
token = enwp.token(pywikibot.Page(enwp, 'Main Page'), 'protect')


def should_we_protect(p_status, tmrw):
    if not p_status:
        print 'Unprotected. Will protect.'
        return True
    move = p_status.get('move', None)
    edit = p_status.get('edit', None)
    if not move:
        print 'Not move protected. Will protect.'
        return True
    if move['level'] != 'sysop':
        print 'Not sysop-protected.'
        return True
    if move['expiry'] == 'infinity':
        print 'Indefinitely protect. Will skip.'
        return False
    ts = pywikibot.Timestamp.fromISOformat(move['expiry'])
    if ts < tmrw:
        print 'Expires before it is off the main page. Will protect'
        return True
    print 'Looks good to me. +2'
    return False


def protect(page, tmrw, p_status):
    expiry = tmrw.strftime("%Y-%m-%dT%H:%M:%SZ")
    params = {'action': 'protect',
              'title': page.title(),
              'token': token,
              'protections': 'move=sysop',
              'expiry': expiry,
              'reason': 'Upcoming TFA ([[WP:BOT|bot protection]])',
              }
    if 'edit' in p_status:
        params['protections'] += '|edit=' + p_status['edit']['level']
        params['expiry'] += '|' + p_status['edit']['expiry']
    req = api.Request(site=enwp, **params)
    data = req.submit()
    print data


def prot_status(page):
    #action=query&titles=Albert%20Einstein&prop=info&inprop=protection|talkid&format=jsonfm
    params = {'action': 'query',
              'titles': page.title(),
              'prop': 'info',
              'inprop': 'protection',
              }
    req = api.Request(site=enwp, **params)
    data = req.submit()
    d = data['query']['pages'].values()[0]
    p = {}
    if 'protection' in d:
        for a in d['protection']:
            p[a['type']] = a
    return p


def do_page(date):
    date_plus_one = date + datetime.timedelta(days=1)
    d_plus_one = datetime.datetime(date_plus_one.year, date_plus_one.month, date_plus_one.day)
    d = datetime.datetime(date.year, date.month, date.day)
    dt = d.strftime('%B %d, %Y').replace(' 0', ' ')  # Strip the preceding 0
    pg = pywikibot.Page(enwp, 'Template:TFA title/' + dt)
    if not pg.exists():
        return None
    title = pg.get()
    if not title:
        return None
    tfa = pywikibot.Page(enwp, title)
    if tfa.isRedirectPage():
        #do something
        pass
        return True
    else:
        p_status = prot_status(tfa)
        if should_we_protect(p_status, d_plus_one):
            protect(tfa, d_plus_one, p_status)
            return True


def main():
    d = datetime.date.today() + datetime.timedelta(days=1)
    go = True
    while go:
        f = do_page(d)
        if f is None:
            go = False
        d += datetime.timedelta(days=1)



bot = Defender()
bot.run()
