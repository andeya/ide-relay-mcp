import js from "@eslint/js";
import tsParser from "@typescript-eslint/parser";
import pluginVue from "eslint-plugin-vue";
import vueParser from "vue-eslint-parser";

/** ESLint flat config: Vue SFCs (TS in script) + shared JS rules. */
export default [
  js.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    files: ["**/*.vue"],
    languageOptions: {
      parser: vueParser,
      parserOptions: {
        parser: tsParser,
        ecmaVersion: "latest",
        sourceType: "module",
      },
    },
  },
  {
    rules: {
      "vue/multi-word-component-names": "off",
      "vue/max-attributes-per-line": "off",
      "vue/singleline-html-element-content-newline": "off",
      "vue/html-indent": "off",
      "vue/html-self-closing": "off",
      "vue/html-closing-bracket-newline": "off",
      "vue/multiline-html-element-content-newline": "off",
      "vue/no-v-html": "off",
    },
  },
  { ignores: ["dist/**", "src-tauri/**", "node_modules/**"] },
];
