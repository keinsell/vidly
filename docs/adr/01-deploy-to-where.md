# Where we want to deploy?

W momencie kiedy zastanawialiśmy się nad decyzją gdzie powinniśmy puścić naszą aplikację posiadaliśmy bardzo minimalistyczy plik binarny który był docelowo tworzony na platformę linux. Posiadaliśmy wiele kont u różnych dostarczycieli usług internetowych, chcieliśmy uniknąć sytuacji gdzie nasz dostarczyciel chmury dyktował nam warunki oraz kierunku jak powinna być tworzona nasza aplikacja - byliśmy natomiast otwarci na współpracę z nim.

Najbardziej podręcznym woborem okazał się **Amazon Web Services Lightsail** przy którym rozważyliśmy deployment naszej aplikacji jako kontener w najtańszej możliwej specyfikacji (7\$ na miesiąc) a zaraz potem prywatny serwer gdzie cena była niższa o 2\$ oraz byliśmy w stanie zaoszczędzić 2.5\$ kiedy nasz serwer nie posiadałby publicznego adresu ipv4 ale posiadał by publiczny adres IPV6.

Zadecydowaliśmy aby wybrać **prywatną maszynę wirtualną** oferowaną przez **Amazon Web Services Lightsail** w najtańszej z możliwych konfiguracji (z zastosowaniem publicznego adresu IPv4 ze względu na lepsze zajnajomienie osoby prowadzącej projekt), specyfikacja maszyny nie miała znaczenia w podejmowanym przez nas wyborze ze względu na zbyt dużą ilość niewiadomych czynników. Uprzednio wybrana maszyna używała **openSUSE 16.0** jako główny system operacyjny ze względu na "zaznajomienie" głównego kontrybutora.
