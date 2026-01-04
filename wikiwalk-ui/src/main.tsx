import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'
import './i18n'
import * as Sentry from "@sentry/react";
import { ErrorBoundary } from './ErrorBoundary.tsx';
import { ThemeProvider } from './ThemeProvider.tsx';

import {
  createBrowserRouter,
  RouterProvider,
} from "react-router-dom";
import { loadPaths } from './loaders.ts';

// Initialize Sentry
if (import.meta.env.VITE_SENTRY_DSN) {
  Sentry.init({
    dsn: import.meta.env.VITE_SENTRY_DSN,
    integrations: [
      Sentry.browserTracingIntegration(),
      Sentry.replayIntegration(),
    ],
    tracesSampleRate: 1.0,
    replaysSessionSampleRate: 0.1,
    replaysOnErrorSampleRate: 1.0,
  });
}

const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    loader: loadPaths
  },
  {
    path: "/paths/:sourceId/:targetId",
    element: <App />,
    loader: loadPaths
  },
]);

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ThemeProvider>
        <React.Suspense fallback={<div>Loading...</div>}>
          <RouterProvider router={router} />
        </React.Suspense>
      </ThemeProvider>
    </ErrorBoundary>
  </React.StrictMode>,
)
