module.exports = {
    env: {
        browser: true,
        es2021: true,
        node: false
    },
    extends: [
        'eslint:recommended'
    ],
    parserOptions: {
        ecmaVersion: 2021,
        sourceType: 'script'
    },
    globals: {
        Chart: 'readonly',
        d3: 'readonly',
        ChartFactory: 'writable',
        DataService: 'writable',
        ResponsiveLayout: 'writable',
        MooseNGDashboard: 'writable'
    },
    rules: {
        'indent': ['error', 4],
        'linebreak-style': ['error', 'unix'],
        'quotes': ['error', 'single'],
        'semi': ['error', 'always'],
        'no-unused-vars': ['warn'],
        'no-console': ['off'],
        'no-debugger': ['warn'],
        'prefer-const': ['error'],
        'no-var': ['error'],
        'arrow-spacing': ['error'],
        'brace-style': ['error', '1tbs'],
        'comma-dangle': ['error', 'never'],
        'comma-spacing': ['error'],
        'comma-style': ['error'],
        'computed-property-spacing': ['error'],
        'func-call-spacing': ['error'],
        'key-spacing': ['error'],
        'keyword-spacing': ['error'],
        'object-curly-spacing': ['error', 'always'],
        'space-before-blocks': ['error'],
        'space-before-function-paren': ['error', 'never'],
        'space-in-parens': ['error'],
        'space-infix-ops': ['error'],
        'spaced-comment': ['error'],
        'eqeqeq': ['error', 'always'],
        'no-eval': ['error'],
        'no-implied-eval': ['error'],
        'no-new-func': ['error'],
        'radix': ['error'],
        'no-shadow': ['error'],
        'no-undef-init': ['error'],
        'no-undefined': ['error'],
        'camelcase': ['error'],
        'new-cap': ['error'],
        'no-array-constructor': ['error'],
        'no-new-object': ['error']
    }
};