"""Point 4 : réponse HTTP d'erreur renvoyée comme URL ; aucun timeout pycurl."""

import importlib
import sys
import types

import pytest


def _faux_pycurl(code, corps):
    m = types.ModuleType("pycurl")
    m.error = type("error", (Exception,), {})
    opts = []

    class Curl:
        for i, n in enumerate(
            "URL UPLOAD INFILESIZE READDATA NOPROGRESS XFERINFOFUNCTION VERBOSE "
            "WRITEFUNCTION TIMEOUT CONNECTTIMEOUT LOW_SPEED_LIMIT LOW_SPEED_TIME "
            "FAILONERROR RESPONSE_CODE".split()
        ):
            locals()[n] = i

        def setopt(self, k, v):
            opts.append(k)
            if k == self.WRITEFUNCTION:
                self._w = v

        def perform(self):
            self._w(corps)

        def getinfo(self, k):
            return code

        def close(self):
            pass

    m.Curl = Curl
    m.opts, m.C = opts, Curl
    return m


@pytest.fixture
def uploader(monkeypatch, dossier_propre):
    def charge(code=200, corps=b"https://curl.libriciel.fr/abc.mp4"):
        fp = _faux_pycurl(code, corps)
        monkeypatch.setitem(sys.modules, "pycurl", fp)
        sys.modules.pop("utils.curl_uploader", None)
        mod = importlib.import_module("utils.curl_uploader")
        return mod, fp

    return charge


def _fichier(dossier):
    p = f"{dossier}/clip.mp4"
    with open(p, "wb") as f:
        f.write(b"abc")
    return p


def test_reponse_500_n_est_pas_une_url(uploader, dossier_propre):
    mod, _ = uploader(500, b"<html>500 Internal Server Error</html>")
    with pytest.raises(Exception):
        res = mod.upload_large_file_via_curl(_fichier(dossier_propre))
        pytest.fail(f"réponse 500 renvoyée comme URL de partage : {res!r}")


def test_timeout_pycurl_defini(uploader, dossier_propre):
    mod, fp = uploader()
    mod.upload_large_file_via_curl(_fichier(dossier_propre))
    C = fp.C
    assert {C.TIMEOUT, C.CONNECTTIMEOUT, C.LOW_SPEED_TIME} & set(fp.opts), (
        "aucun timeout pycurl"
    )
