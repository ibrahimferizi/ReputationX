import express from 'express';
import axios from 'axios';
import cors from 'cors';

const app = express();
const PORT = 3001;

// Enable CORS for all origins (for demo)
app.use(cors());

app.use(express.json());

// Proxy endpoint for Identity Prism API
app.get('/api/reputation', async (req, res) => {
  const { address } = req.query;

  if (!address) {
    return res.status(400).json({ error: 'Wallet address is required' });
  }

  try {
    const response = await axios.get(
      `https://identityprism.xyz/api/reputation`,
      {
        params: { address },
        timeout: 10000,
      }
    );

    res.json(response.data);
  } catch (error) {
    console.error('Error fetching reputation:', error.message);
    res.status(500).json({ 
      error: 'Failed to fetch reputation data',
      details: error.message 
    });
  }
});

app.listen(PORT, () => {
  console.log(`Backend proxy running on http://localhost:${PORT}`);
});
