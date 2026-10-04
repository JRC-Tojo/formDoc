// vite.config.ts の define で埋め込む値
/** アプリの版（ui/package.json の version） */
declare const __APP_VERSION__: string;
/** ビルドの識別子（CI ではコミットの SHA。Web版の更新検知に使う） */
declare const __BUILD_ID__: string;
