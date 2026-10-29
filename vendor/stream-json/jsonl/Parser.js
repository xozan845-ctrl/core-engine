'use strict';

const { Transform } = require('stream');

/**
 * Parser JSONL mínimo compatible con `stream-json/jsonl/Parser.js` para el
 * consumidor CJS de minio (notification poller). Emite `{key, value}` por línea
 * JSON y soporta fragmentos arbitrarios entre chunks.
 */
class JsonlParser extends Transform {
  static make(options) {
    return new JsonlParser(options);
  }

  constructor(options) {
    super({ ...(options ?? {}), readableObjectMode: true });
    this._rest = '';
    this._counter = 0;
  }

  _transform(chunk, encoding, callback) {
    const text = this._rest + chunk.toString('utf8');
    const lines = text.split(/\r?\n/);
    this._rest = lines.pop() ?? '';
    try {
      for (const line of lines) {
        if (line.trim()) {
          this.push({ key: this._counter++, value: JSON.parse(line) });
        }
      }
      callback();
    } catch (error) {
      callback(error);
    }
  }

  _flush(callback) {
    try {
      if (this._rest.trim()) {
        this.push({ key: this._counter++, value: JSON.parse(this._rest) });
      }
      this._rest = '';
      callback();
    } catch (error) {
      callback(error);
    }
  }
}

module.exports = JsonlParser;
module.exports.make = JsonlParser.make;
