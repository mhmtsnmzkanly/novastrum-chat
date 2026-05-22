/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        matte: {
          950: "#0d0d0b",
          900: "#161512",
          800: "#222019",
        },
        ember: {
          300: "#ffd37a",
          400: "#f4ad38",
          500: "#d88a18",
        },
      },
      boxShadow: {
        brutal: "6px 6px 0 #000000",
      },
    },
  },
  plugins: [],
};
