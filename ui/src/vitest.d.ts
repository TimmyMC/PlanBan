// Make the jest-dom matchers (toBeInTheDocument, …) visible to tsc, not just
// at runtime via vitest.setup.ts (which lives outside the tsconfig `src` root).
import "@testing-library/jest-dom/vitest";
