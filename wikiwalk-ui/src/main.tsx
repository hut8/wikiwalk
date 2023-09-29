import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'

import {
  createBrowserRouter,
  RouterProvider,
} from "react-router-dom";
import { loadPaths } from './loaders.ts';

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
    <RouterProvider router={router} />
  </React.StrictMode>,
)
