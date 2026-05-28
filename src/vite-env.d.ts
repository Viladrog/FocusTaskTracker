/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APP_DEBUG?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
