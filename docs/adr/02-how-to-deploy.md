# How we want to deploy this?

Rozważałem wiele metod na wdrążanie aplikacji do środowiska chmurowego, najprostszą wstępną opcją było pobieranie repozytorium na docelowym serwerze poprzez dostęp po SSH za każdym razem kiedy chicałem zmienić wersję aplikacji, ale pomyślałem że z gwałtowym rozwojem aplikacji będzie to męczące.

Kolejną opcją okazuje się być wdrążenie wcześniej wspomnianej metody poprzez GitHub Actions co znacząco ułatwia proces wdrążania oraz zapewnia dokumentację jak sam proces przebiega, ale nadal coś mi tutaj nie grało na szerszą skalę.

Brakowało mi specyfikacji przygotowania serwera, nie ukrywam że w pierwszym momencie kiedy się do niego zalogowałem brakowało mi paczek `rust` oraz `git`, owszem był to jednorazowy przypadek ale nadal mógł być istotny, podobnie jak zosstawienie miejsca na reverse proxy na naszej maszynie wirtualej czy zarządzenie naszą aplikacją poprzez systemd bądź konteneryzację.

Po stworzeniu maszyny wirtualnej umieściliśmy konfigurację sekretów w GitHub Actions które zawierały `MACHINE_IP`, `SSH_PRIVATE_KEY` oraz `MACHINE_USERNAME` które wykorzystaliśmy przy specyfikacji procesu.

Pomyśleliśmy że najprostszym rozwiązaniem w tym przypadku może być przygotownie prostego skryptu ukazującego przygotowanie maszyny wirtualnej pod uruchomienie na niej serwisu odpowiedzialnego za naszą aplikację oraz dodanie takowego do mendadżera procesu początkowego (`systemd`).

```
sudo nano /etc/systemd/system/vidly.service
```

```
[Unit]
Description=Vidly web server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/vidly
ExecStart=/opt/vidly/vidly
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

```
sudo systemctl enable vidly.service
sudo systemctl start vidly.service
sudo systemctl status vidly.service
```
