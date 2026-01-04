import React, { Component, ReactNode } from 'react';
import toast, { Toaster } from 'react-hot-toast';
import * as Sentry from '@sentry/react';
import { Box, Button, Typography } from '@mui/material';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    // Log to Sentry
    Sentry.captureException(error, {
      contexts: {
        react: {
          componentStack: errorInfo.componentStack,
        },
      },
    });

    // Show toast notification
    toast.error('An error occurred while rendering the graph. Please try again.', {
      duration: 6000,
      position: 'top-center',
      style: {
        background: '#ef4444',
        color: '#fff',
        padding: '16px',
        borderRadius: '8px',
      },
    });

    console.error('Error caught by boundary:', error, errorInfo);
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
    window.location.href = '/';
  };

  render() {
    if (this.state.hasError) {
      return (
        <>
          <Toaster />
          <Box
            sx={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              minHeight: '400px',
              padding: 4,
              textAlign: 'center',
            }}
          >
            <Typography variant="h5" gutterBottom>
              Something went wrong
            </Typography>
            <Typography variant="body1" color="text.secondary" sx={{ mb: 3, maxWidth: 600 }}>
              We encountered an error while displaying the graph. This has been reported and we'll look into it.
            </Typography>
            {this.state.error && (
              <Typography
                variant="body2"
                sx={{
                  mb: 3,
                  p: 2,
                  backgroundColor: 'rgba(239, 68, 68, 0.1)',
                  borderRadius: 1,
                  fontFamily: 'monospace',
                  fontSize: '0.875rem',
                  maxWidth: 600,
                  wordBreak: 'break-word',
                }}
              >
                {this.state.error.message}
              </Typography>
            )}
            <Button variant="contained" onClick={this.handleReset}>
              Return to Home
            </Button>
          </Box>
        </>
      );
    }

    return (
      <>
        <Toaster />
        {this.props.children}
      </>
    );
  }
}
