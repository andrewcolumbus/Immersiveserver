const express = require('express');
const path = require('path');

const app = express();
const PORT = 2900;

// Serve static files from current directory
app.use(express.static(__dirname));

// Serve index.html for root
app.get('/', (req, res) => {
  res.sendFile(path.join(__dirname, 'index.html'));
});

app.listen(PORT, () => {
  console.log(`Dashboard server running at http://localhost:${PORT}`);
});
