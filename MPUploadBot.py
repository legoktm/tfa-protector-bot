#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Public domain
from __future__ import unicode_literals

import hashlib
import pywikibot
from pywikibot.data import api
from pywikibot import config
import requests
import sys

import TFAdefenderbot

enwp = pywikibot.Site('en', 'wikipedia')  # Note overridden below.
commons = pywikibot.Site('commons', 'commons')
username = 'TFA Protector Bot'


def gen():
    page = pywikibot.Page(enwp, 'User:TFA Protector Bot/watch.js')
    text = page.get()
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith('#'):
            yield pywikibot.Page(enwp, line)


def main():
    imgs = set()
    for page in gen():
        for img in page.imagelinks():
            imgs.add(img)

    for img in imgs:
        filename = img.title(withNamespace=False)
        if should_we_upload(filename):
            reupload(filename)
        should_we_protect(filename)

    # Okay, time to cleanup now.
    req = api.Request(
        site=enwp,
        action='query',
        list='watchlistraw',
        wrlimit='max',
        wrnamespace=6
    )
    data = req.submit()
    for page in data['watchlistraw']:
        w_img = pywikibot.ImagePage(enwp, page['title'])
        if not w_img in imgs:  # Aka no longer on the main page
            cleanup(w_img.title(withNamespace=False))


def cleanup(filename):
    w_img = pywikibot.ImagePage(enwp, 'File:' + filename)
    hist = w_img.getVersionHistory()
    delete = True
    timestamps = set()
    for rev in hist:
        if rev[2] != username:
            timestamps.add(rev[1].totimestampformat())
            delete = False
            break
    if delete:
        w_img.delete(reason='Bot: Image is no longer on main page', prompt=False)
    else:
        # Okay so POTDs might have a local description page that we overwrote
        # So lets delete the page, then restore the revisions that aren't ours.
        w_img.delete(reason='Bot: Image is no longer on main page', prompt=False)
        req = api.Request(
            site=enwp,
            action='undelete',
            title=w_img.title(),
            reason='Undeleting previous history',
            timestamps=list(timestamps),
            token=enwp.token(pywikibot.Page(enwp, 'Main Page'), 'edit')
        )
        print req.submit()
    w_img.watch(unwatch=True)


def check_if_has_images(filename):
    req = api.Request(site=enwp, action='query', prop='imageinfo', titles='File:' + filename)
    data = req.submit()
    return 'imageinfo' in data['query']['pages'].values()[0]


def should_we_upload(filename):
    w_img = pywikibot.ImagePage(enwp, 'File:' + filename)
    if not w_img.exists():
        return True
    if not check_if_has_images(filename):
        return True
    # At this point there's an image, it may not be the same as
    # commons, but we don't need to upload anything.
    return False


def should_we_protect_internal(p_status):
    #redirect is true if we should also check for edit protection.
    #will return a dict of {'type':{'level':'sysop','expiry':ts}}
    return_this = {}
    upload = p_status.get('upload', None)
    if upload:
        if upload['level'] != 'sysop':
            return_this['upload'] = {'level': 'sysop', 'expiry': 'infinity'}
        elif upload['expiry'] != 'infinity':
            return_this['upload'] = {'level': 'sysop', 'expiry': 'infinity'}
    else:
        return_this['upload'] = {'level': 'sysop', 'expiry': 'infinity'}
    return return_this


def should_we_protect(filename):
    w_img = pywikibot.ImagePage(enwp, 'File:' + filename)
    p_status = TFAdefenderbot.prot_status(w_img)
    protect_this = should_we_protect_internal(p_status)
    if protect_this:
        TFAdefenderbot.protect(w_img, p_status, protect_this)
        w_img.watch()


def sha1check(fname):
    """
    Return the sha1 value of a local file
    http://stackoverflow.com/questions/7829499/using-hashlib-to-compute-md5-digest-of-a-file-in-python3
    """
    with open(fname, mode='rb') as f:
        d = hashlib.sha1()
        for buf in f.read(128):
            d.update(buf)
    return d.hexdigest()


def reupload(filename):
    c_img = pywikibot.ImagePage(commons, 'File:' + filename)
    w_img = pywikibot.ImagePage(enwp, 'File:' + filename)
    extension = filename.split('.')[-1]
    url = c_img.fileUrl()
    print url
    sha1 = c_img.getFileSHA1Sum()
    print sha1
    fname = sha1 + '.' + extension
    print 'Downloading image'
    r = requests.get(url, stream=True)
    with open(fname, 'wb') as f:
        for chunk in r.iter_content(1024):
            f.write(chunk)
    print 'Saved to {0}'.format(fname)
    localsha1 = sha1check(fname)
    if sha1 != localsha1:
        print 'ERROR: DID NOT DOWNLOAD FILE PROPERLY. Expected {0}. Local image is {1}.'.format(sha1, localsha1)
        sys.exit(1)
    text = c_img.get()
    newtext = '{{Uploaded from Commons}}\n\n' + text
    print newtext
    enwp.upload(w_img,
                source_filename=fname,
                comment='Bot: Uploading image that will soon be on the Main Page',
                text=newtext,
                ignore_warnings=True,  # Mehhhhhhh
                watch=True,
    )


if __name__ == '__main__':
    config.put_throttle = 0
    config.maxlag = 999999999  # Don't worry about it
    account_name = 'TFA Protector Bot'
    enwp = pywikibot.Site('en', 'wikipedia', account_name)
    enwp.login()

    main()
