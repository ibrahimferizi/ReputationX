const TIERS = [
  { name: 'Mercury', minScore: 0 },
  { name: 'Venus', minScore: 120 },
  { name: 'Earth', minScore: 280 },
  { name: 'Mars', minScore: 450 },
  { name: 'Jupiter', minScore: 650 },
  { name: 'Saturn', minScore: 850 },
  { name: 'Uranus', minScore: 1050 },
  { name: 'Neptune', minScore: 1200 },
  { name: 'Sun', minScore: 1320 },
];

const MAX_SCORE = 1400;

/**
 * @param {number} score
 * @returns {{ score: number, tier: string, celestila_tier: string }}
 */
export function scoreToTier(score) {
  const clamped = Math.max(0, Math.min(MAX_SCORE, Math.round(score)));
  let tier = TIERS[0].name;

  for (const entry of TIERS) {
    if (clamped >= entry.minScore) {
      tier = entry.name;
    }
  }

  return {
    score: clamped,
    tier,
    celestila_tier: tier,
  };
}

export { MAX_SCORE };
