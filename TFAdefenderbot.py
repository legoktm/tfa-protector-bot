#!/usr/bin/env python
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

class Defender:
    def __init__(self):
        self.enwp = pywikibot.Site('en', 'wikipedia')
        self.page = None
        self.today = datetime.date.today()
        self.tomorrow = self.today + datetime.timedelta(days=1)
        self.tmrw = datetime.datetime(self.tomorrow.year, self.tomorrow.month, self.tomorrow.day)


    def prot_status(self):
        #action=query&titles=Albert%20Einstein&prop=info&inprop=protection|talkid&format=jsonfm
        params = {'action': 'query',
                  'titles': self.page.title(),
                  'prop': 'info',
                  'inprop': 'protection',
                  'intoken': 'protect',
                  }
        req = api.Request(site=self.enwp, **params)
        data = req.submit()
        d = data['query']['pages'].values()[0]
        #self.token = d['protecttoken']
        self.prot_level = d

    def should_we_protect(self):
        if not self.prot_level['protection']:
            print 'Unprotected. Will protect.'
            return True
        move = False
        for p in self.prot_level['protection']:
            if p['type'] == 'move':
                move = p
                break

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
        if ts < self.tmrw:
            print 'Expires before it is off the main page. Will protect'
            return True
        print 'Looks good to me. +2'
        return False

    def get_tfa(self):
        pass
        #Template:TFA title/April 21, 2013
        dt = self.tmrw.strftime('%B %d, %Y').replace(' 0', ' ')  # Strip the preceding 0
        pg = pywikibot.Page(self.enwp, 'Template:TFA title/' + dt)
        title = pg.get().strip()
        print title
        self.page = pywikibot.Page(self.enwp, title)

    def protect(self):
        expiry = (self.tmrw + datetime.timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%SZ")
        params = {'action': 'protect',
                  'title': self.page.title(),
                  'token': self.token,
                  'protections': 'move=sysop',
                  'expiry': expiry,
                  'reason': 'Upcomping TFA ([[WP:BOT|bot protection]])',
                  }
        req = api.Request(site=self.enwp, **params)
        data = req.submit()
        print data

    def run(self):
        self.get_tfa()
        self.prot_status()
        if self.should_we_protect():
            self.protect()


bot = Defender()
bot.run()