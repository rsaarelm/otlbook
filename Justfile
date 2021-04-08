test:
    cargo test --all

install-ankiconnect:
    #!/usr/bin/env sh
    if [ ! -d ~/.local/share/Anki2/addons21/2055492159 ]
    then
        echo "AnkiConnect plugin not found, installing..."
        mkdir -p ~/.local/share/Anki2/addons21
        pushd $(mktemp -d)
        git clone https://github.com/FooSoft/anki-connect/
        mv anki-connect/plugin ~/.local/share/Anki2/addons21/2055492159
        popd
    else
        echo "AnkiConnect is already installed."
    fi
