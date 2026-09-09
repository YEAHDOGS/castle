# Uptime pinger

Zero-token website monitor for Castle blocks. No AI, no agent — just curl,
once a minute, appending to `~/uptime.log`.

Install on the Castle host:
```
chmod +x uptime-ping.sh
crontab -e   # add:  * * * * * /path/to/uptime-ping.sh
```

Edit the `URLS` array in the script to add/remove sites.
