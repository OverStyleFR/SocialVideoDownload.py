import struct

def what(filename, h=None):
    if h is None:
        with open(filename, 'rb') as f:
            h = f.read(32)
    if h is None or len(h) < 8:
        return None
    if h.startswith(b'\x89PNG\r\n\x1a\n'):
        return 'png'
    if h.startswith(b'\xff\xd8'):
        return 'jpeg'
    if h.startswith(b'GIF87a') or h.startswith(b'GIF89a'):
        return 'gif'
    if h.startswith(b'RIFF') and h[8:12] == b'WEBP':
        return 'webp'
    return None
