#!/usr/bin/env python
from __future__ import unicode_literals
"""
Copyright (C) 2013 Legoktm

Released as CC-Zero.
"""
import datetime
import re
import pywikibot
from pywikibot.data import api
from pywikibot import config

config.put_throttle = 0
config.maxlag = 999999999  # Don't worry about it

account_name = 'Legoktm'

enwp = pywikibot.Site('en', 'wikipedia', account_name)
enwp.login()
token = enwp.token(pywikibot.Page(enwp, 'Main Page'), 'protect')


def should_we_protect(p_status, tmrw, redirect=False):
    #redirect is true if we should also check for edit protection.
    #will return a dict of {'type':{'level':'sysop','expiry':ts}}
    return_this = {}
    move = p_status.get('move', None)
    if move:
        if move['level'] != 'sysop':
            return_this['move'] = {'level': 'sysop', 'expiry': tmrw}
        elif move['expiry'] != 'infinity':
            ts = pywikibot.Timestamp.fromISOformat(move['expiry'])
            if ts < tmrw:
                #expires before off main page.
                return_this['move'] = {'level': 'sysop', 'expiry': tmrw}
    else:
        return_this['move'] = {'level': 'sysop', 'expiry': tmrw}
    edit = p_status.get('edit', None)
    if redirect:
        if edit:
            if edit['level'] != 'sysop':
                return_this['edit'] = {'level': 'sysop', 'expiry': tmrw}
            elif edit['expiry'] != 'infinity':
                ts = pywikibot.Timestamp.fromISOformat(edit['expiry'])
                if ts < tmrw:
                    return_this['edit'] = {'level': 'sysop', 'expiry': tmrw}
        else:
            return_this['edit'] = {'level': 'sysop', 'expiry': tmrw}
    return return_this


def protect(page, p_status, protect_this):
    params = {'action': 'protect',
              'title': page.title(),
              'token': token,
              'protections': [],  # pwb will convert these to a string later on
              'expiry': [],
              'reason': 'Upcoming TFA ([[WP:BOT|bot protection]] - [[Wikipedia:Bots/Requests for approval/TFA Protector Bot|BRFA]])',
              }
    for p_type in protect_this:
        params['protections'].append('{0}={1}'.format(p_type, protect_this[p_type]['level']))
        params['expiry'].append(protect_this[p_type]['expiry'].strftime("%Y-%m-%dT%H:%M:%SZ"))
    for p_type in p_status:
        if 'cascade' in p_status[p_type]:
            params['cascade'] = '1'  # send it back i guess?

        if p_type in protect_this:
            #dont try to protect what we want to change
            continue
        if 'source' in p_status[p_type]:
            #skip cascading protection
            continue
        params['protections'] += '|{0}={1}'.format(p_type, p_status[p_type]['level'])
        params['expiry'] += '|' + p_status[p_type]['expiry']
    print params
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
    print d
    p = {}
    if 'protection' in d:
        for a in d['protection']:
            p[a['type']] = a
    return p


def extract_from_tfa(dt):
    pg = pywikibot.Page(enwp, 'Wikipedia:Today\'s featured article/' + dt)
    if not pg.exists():
        return None
    text = pg.get().strip()
    if text == '{{TFAempty}}':
        return None
    # Stole these regexes from Anomie
    m = re.search("'''\s*\[\[\s*([^|\]]+?)\s*(?:\|[^]]+)?\]\][a-z]*\s*'''", text)
    if not m:
        m = re.search("<b>\s*\[\[\s*([^|\]]+?)\s*(?:\|[^]]+)?\]\]\s*</b>", text)
    if not m:
        return None
    title = m.group(1)
    # Use the API to normalize it...
    r = api.Request(site=enwp, action='query', titles=title)
    data = r.submit()
    title = data['query']['pages'].values()[0]['title']
    return title


def do_page(date):
    date_plus_one = date + datetime.timedelta(days=1)
    d_plus_one = datetime.datetime(date_plus_one.year, date_plus_one.month, date_plus_one.day)
    d = datetime.datetime(date.year, date.month, date.day)
    dt = d.strftime('%B %d, %Y').replace(' 0', ' ')  # Strip the preceding 0
    pg = pywikibot.Page(enwp, 'Template:TFA title/' + dt)
    print pg
    title = None
    if not pg.exists():
        print str(pg) + ' doesnt exist.'
        print 'Checking for a TFA subpage...'
        title = extract_from_tfa(dt)
        if not title:
            # bail
            return None
    if not title:
        title = pg.get()
    if not title:
        print str(pg) + ' is empty.'
        return None
    tfa = pywikibot.Page(enwp, title)
    p_status = prot_status(tfa)
    if tfa.isRedirectPage():
        print 'Uhoh, is a redirect.'
        real_tfa = tfa.getRedirectTarget()
        real_p_status = prot_status(real_tfa)
        protect_this_redirect = should_we_protect(p_status, d_plus_one, redirect=True)
        if protect_this_redirect:
            protect(tfa, p_status, protect_this_redirect)
        real_protect_this = should_we_protect(p_status, d_plus_one)
        if real_protect_this:
            protect(real_tfa, real_p_status, real_protect_this)
        return True
    else:
        protect_this = should_we_protect(p_status, d_plus_one)
        if protect_this:
            protect(tfa, p_status, protect_this)
            return True
        else:
            print 'Already protected.'
            print False
    return True


def main():
    d = datetime.date.today() + datetime.timedelta(days=1)
    go = True
    while go:
        f = do_page(d)
        if f is None:
            go = False
        d += datetime.timedelta(days=1)



if __name__ == "__main__":
    main()
