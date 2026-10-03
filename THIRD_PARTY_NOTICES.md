# Componentes de terceiros

O executável `proton-drive` é um binário oficial da Proton distribuído sem modificações. Sua versão, origem HTTPS e SHA-512 estão registrados em `bin/release.json`. Ele não é de autoria deste projeto.

Consulte o [repositório oficial Proton Drive SDK](https://github.com/ProtonDriveApps/sdk) para o código-fonte, licenças e condições aplicáveis ao CLI e seus componentes. A distribuição deste aplicativo não altera essas condições nem as condições de uso do serviço Proton Drive.

Tauri, Nuxt, Vue, DaisyUI, Tailwind CSS, Lucide e as dependências listadas em `package-lock.json` e `Cargo.lock` mantêm suas respectivas licenças. As marcas Proton e Proton Drive pertencem aos seus titulares. Este projeto é independente.

PDF.js (`pdfjs-dist`), Mozilla Foundation and contributors, Apache-2.0. https://github.com/mozilla/pdf.js


O executável `proton-drive-computers` é uma compilação independente e identificada como `external-drive-desktop` a partir dos componentes CLI/SDK oficiais sob a licença em `bin/PROTON-SDK-LICENSE.md`, com comandos adicionais deste projeto para registrar computadores e consultar lotes de metadados. Não é um binário oficial da Proton. Fonte-base: https://github.com/ProtonDriveApps/sdk/tree/5491f2eea473acaaa86b5969774b84610a37bd46. Adaptações e procedimento de compilação: `scripts/computers/` e `scripts/build-computers.mjs`. Bun 1.3.14 (MIT) é incorporado nesse executável: https://github.com/oven-sh/bun/tree/bun-v1.3.14. Os componentes mantêm suas respectivas licenças.
