Parząc na sposób w jaki aplikacja którą posiadamy w repozytorium jest dostarczana zacząłem się zastanawiać nad potrzebą pod reverse proxy które udostępniłoby aplikację na zewnątrz, oczywiście w pewnym stopniu aplikacja którą posiadamy w repozytorium mogłaby zarządzać się sama ale coś podpowiadało mi że jeśli istnieje możliwość bezproblemowego zastosowania reverse proxy w tym miejscu to mogłoby być ono zastosowane.

Postanowiliśmy nie konfigurować obsługi certyfikatów SSL z racji że planowaliśmy wykorzystać posiadaną domenę internetową na serwisie CloudFlare gdzie mogliśmy pozostawić takwą kwestię naszemu dostarczycielowi domeny.

```
sudo zypper install -y haproxy
```

```cfg
global
    log /dev/log local0
    log /dev/log local1 notice
    chroot /var/lib/haproxy
    stats socket /run/haproxy/admin.sock mode 660 level admin expose-fd listeners
    stats timeout 30s
    user haproxy
    group haproxy
    daemon

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    option  forwardfor
    option  http-server-close
    timeout connect 5s
    timeout client  30s
    timeout server  30s

frontend http_in
    bind *:80
    default_backend app

backend app
    balance roundrobin
    server app1 127.0.0.1:3000
```

```
sudo haproxy -c -f /etc/haproxy/haproxy.cfg
sudo systemctl enable --now haproxy
sudo systemctl status haproxy
```
