/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        'bg-primary': '#12131C',
        'bg-secondary': '#1A1C28',
        'bg-tertiary': '#242738',
        'accent-neuro': '#FFB6C1',
        'accent-evil': '#FF0055',
        'accent-vedal': '#98FB98',
        'accent-guest': '#FFD700',
        'accent-chat': '#87CEEB',
        'border-dark': '#2E3247',
      },
      fontFamily: {
        sans: ['"Be Vietnam Pro"', 'Inter', 'sans-serif'],
      },
      boxShadow: {
        acrylic: '0 8px 32px 0 rgba(0, 0, 0, 0.37)',
      },
    },
  },
  plugins: [],
};
