const http = require('node:http');

const { createBroker, handleBrokerRequest } = require('./fixture-broker');

function createServer(options = {}) {
  const broker = options.broker || createBroker(options);
  return http.createServer((req, res) => handleBrokerRequest(broker, req, res));
}

if (require.main === module) {
  const port = Number(process.env.PORT || 8787);
  createServer().listen(port, '127.0.0.1', () => {
    console.log(`fixture broker listening on http://127.0.0.1:${port}`);
  });
}

module.exports = { createServer };
