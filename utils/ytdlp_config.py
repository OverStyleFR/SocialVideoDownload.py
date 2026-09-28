# utils/ytdlp_config.py
# Default yt-dlp options used across the bot.

DEFAULT_YDL_OPTS = {
    'outtmpl': 'downloads/%(title)s.%(ext)s',
    'user_agent': (
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
        'AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36'
    ),
    'http_headers': {
        'User-Agent': (
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
            'AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36'
        ),
        'Referer': 'https://www.tiktok.com/',
    },
}
