import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'
import './i18n'
import * as Sentry from "@sentry/react";

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
    <Sentry.ErrorBoundary fallback={<div>An error has occurred. Please refresh the page.</div>}>
      <RouterProvider router={router} />
    </Sentry.ErrorBoundary>
  </React.StrictMode>,
)
