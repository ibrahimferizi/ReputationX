import express from 'express';
import cors from 'cors';
import { PORT } from './src/config.js';
import { getWalletReputation } from './src/reputation/service.js';

const app = express();

app.use(cors());
app.use(express.json());

app.get('/api/reputation', async (req, res) => {
  const { address } = req.query;

  if (!address || typeof address !== 'string') {
    return res.status(400).json({ error: 'Wallet address is required' });
  }

  try {
    const reputation = await getWalletReputation(address.trim());
    res.json(reputation);
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';

    if (message === 'Invalid Solana wallet address') {
      return res.status(400).json({ error: message });
    }

    console.error('Error fetching reputation:', message);
    res.status(500).json({
      error: 'Failed to fetch reputation data',
      details: message,
    });
  }
});

app.listen(PORT, () => {
  console.log(`Backend running on http://localhost:${PORT}`);
});
