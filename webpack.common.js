const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const { CleanWebpackPlugin } = require('clean-webpack-plugin');
const CopyPlugin = require('copy-webpack-plugin');

module.exports = {
    entry: './webapp/index.js',
    module: {
        rules: [
            {
                test: /\.jsx?$/,
                use: 'babel-loader',
                exclude: /node_modules/,
            },
            {
                test: /\.css$/i,
                use: ['style-loader', 'css-loader'],
            },
        ],
    },
    plugins: [
        new CleanWebpackPlugin(),
        new CopyPlugin([
            { from: "webapp/static", flatten: true },
        ]),
        new HtmlWebpackPlugin({
            title: "Minuteman",
            template: 'webapp/index.html',
        }),
    ],
    resolve: {
        extensions: [ '.js', '.jsx' ],
    },
    output: {
        filename: 'bundle.js',
        publicPath: '/static/',
        path: path.resolve(__dirname, 'static'),
    },
};
